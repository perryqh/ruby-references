use std::path::PathBuf;
use std::sync::Arc;

use futures::future::join_all;
use serde::{Deserialize, Serialize};

use crate::{cache::CacheResult, configuration};

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

// TODO: benchmark with criterion crate
pub async fn parse(
    configuration: &configuration::Configuration,
) -> anyhow::Result<Vec<ProcessedFile>> {
    let cache = Arc::new(configuration.get_cache());

    let futures = configuration.included_files.iter().map(|path| {
        let cache = Arc::clone(&cache);
        async move {
            match cache.get(path).await {
                Ok(CacheResult::Processed(processed_file)) => Ok(processed_file),
                Ok(CacheResult::Miss(empty_cache_entry)) => {
                    let processed_file = process_file(path, configuration)?;
                    cache.write(&empty_cache_entry, &processed_file).await?;
                    Ok(processed_file)
                }
                Err(e) => Err(e),
            }
        }
    });
    let results = join_all(futures).await;
    results.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use crate::common_test::common_test::file_paths;

    use super::*;

    #[tokio::test]
    async fn simple_parse() -> anyhow::Result<()> {
        let included_files = file_paths("tests/fixtures/small-app")?;
        let configuration = configuration::Configuration {
            included_files,
            ..Default::default()
        };
        let result = parse(&configuration).await?;
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
}
