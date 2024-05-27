use std::path::PathBuf;
use std::sync::Arc;

use anyhow::bail;
use serde::{Deserialize, Serialize};
use tokio::task::JoinSet;

use crate::{
    cache::{cache::Cache, CacheResult},
    configuration,
};

use self::processor::process_file;

pub(crate) mod collector;
pub(crate) mod inflector_shim;
pub(crate) mod namespace_calculator;
pub(crate) mod processor;
pub(crate) mod self_reference_filterer;

#[derive(Debug, PartialEq, Eq, Clone, Default, Serialize, Deserialize)]
pub struct Range {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

#[derive(Debug, PartialEq, Clone, Eq)]
pub struct ParsedDefinition {
    pub fully_qualified_name: String,
    pub location: Range,
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProcessedFile {
    pub absolute_path: PathBuf,
    pub unresolved_references: Vec<UnresolvedReference>,
}

#[derive(Debug, PartialEq, Default, Eq, Clone, PartialOrd, Ord, Serialize, Deserialize)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct UnresolvedReference {
    pub name: String,
    pub namespace_path: Vec<String>,
    pub location: Range,
}

pub async fn parse(
    configuration: Arc<configuration::Configuration>,
) -> anyhow::Result<Vec<ProcessedFile>> {
    let cache = configuration.get_cache();
    let mut set = JoinSet::new();

    configuration.included_files.iter().for_each(|path| {
        let cloned_cache = cache.clone();
        let config_clone = configuration.clone();
        set.spawn(from_cache_or_process(
            path.clone(),
            config_clone,
            cloned_cache,
        ));
    });

    let mut processed_files = Vec::with_capacity(set.len());
    while let Some(res) = set.join_next().await {
        match res {
            Ok(processed_file) => match processed_file {
                Ok(processed_file) => processed_files.push(processed_file),
                Err(e) => bail!("Error: {:?}", e),
            },
            Err(e) => bail!("Error: {:?}", e),
        }
    }

    Ok(processed_files)
}

use futures::future::BoxFuture;

fn from_cache_or_process(
    path: PathBuf,
    configuration: Arc<configuration::Configuration>,
    cache: Arc<dyn Cache + Send + Sync>,
) -> BoxFuture<'static, anyhow::Result<ProcessedFile>> {
    Box::pin(async move {
        match cache.get(&path).await {
            Ok(CacheResult::Processed(processed_file)) => Ok(processed_file),
            Ok(CacheResult::Miss(empty_cache_entry)) => {
                let processed_file = process_file(&path, configuration)?;
                cache.write(&empty_cache_entry, &processed_file).await?;
                Ok(processed_file)
            }
            Err(e) => Err(e),
        }
    })
}

#[cfg(test)]
mod tests {
    use crate::{
        cache::{cached_file::CachedFile, delete_cache},
        common_test::common_test::{configuration_for_fixture, file_paths, SIMPLE_APP},
    };

    use super::*;

    #[tokio::test]
    async fn simple_parse() -> anyhow::Result<()> {
        let included_files = file_paths("tests/fixtures/small-app")?;
        let configuration = Arc::new(configuration::Configuration {
            included_files,
            ..Default::default()
        });
        let result = parse(configuration).await?;
        let processed_paths: Vec<&str> = result
            .iter()
            .map(|pfile| pfile.absolute_path.to_str().unwrap())
            .collect();

        assert_eq!(processed_paths.len(), 28);
        assert_eq!(
            processed_paths.contains(
                &PathBuf::from(
                    "tests/fixtures/small-app/app/controllers/application_controller.rb"
                )
                .canonicalize()
                .unwrap()
                .to_str()
                .unwrap()
            ),
            true
        );
        Ok(())
    }

    #[tokio::test]
    async fn test_cache_hit() -> anyhow::Result<()> {
        let configuration = configuration_for_fixture(SIMPLE_APP, true);
        let cache_dir = configuration.cache_directory.clone();
        let file_path = PathBuf::from("tests/fixtures/simple_app/app/company_data/widget.rb");
        delete_cache(PathBuf::from(&cache_dir)).await?;

        let cached_file = CachedFile {
            cache_dir: PathBuf::from(&cache_dir),
        };
        let cache_result = cached_file.get(&file_path).await;
        assert!(cache_result.is_ok());
        match cache_result.unwrap() {
            CacheResult::Miss(empty_cache_entry) => {
                let processed_file = process_file(&file_path, configuration.clone())?;
                let cache = configuration.get_cache();
                cache.write(&empty_cache_entry, &processed_file).await?;
            }
            _ => {
                assert!(false)
            }
        }
        let cache_result = cached_file.get(&file_path).await;
        assert!(cache_result.is_ok());
        match cache_result.unwrap() {
            CacheResult::Miss(_) => assert!(false),
            CacheResult::Processed(processed_file) => {
                assert_eq!(processed_file.absolute_path, file_path);
            }
        }   

        Ok(())
    }
}
