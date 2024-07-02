use std::path::PathBuf;

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
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

pub fn parse(configuration: &configuration::Configuration) -> anyhow::Result<Vec<ProcessedFile>> {
    let cache = configuration.get_cache();

    configuration
        .included_files
        .par_iter()
        .map(|path| -> anyhow::Result<ProcessedFile> {
            match cache.get(path) {
                Ok(CacheResult::Processed(processed_file)) => Ok(processed_file),
                Ok(CacheResult::Miss(empty_cache_entry)) => {
                    let processed_file = process_file(path, configuration)?;
                    cache.write(&empty_cache_entry, &processed_file)?;
                    Ok(processed_file)
                }
                Err(e) => Err(e),
            }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::{
        cache::Cache,
        cached_file::CachedFile,
        common_test::common_test::{configuration_for_fixture, file_paths, SIMPLE_APP},
    };

    use super::*;

    #[test]
    fn simple_parse() -> anyhow::Result<()> {
        let included_files = file_paths("tests/fixtures/small-app")?;
        let configuration = configuration::Configuration {
            included_files,
            ..Default::default()
        };
        let result = parse(&configuration)?;
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
    #[test]
    fn test_cache_hit() -> anyhow::Result<()> {
        let configuration = configuration_for_fixture(SIMPLE_APP, true);
        let cache_dir = configuration.cache_directory.clone();
        let file_path = PathBuf::from("tests/fixtures/simple_app/app/company_data/widget.rb");
        let _ = configuration.delete_cache();

        let cached_file = CachedFile {
            cache_dir: PathBuf::from(&cache_dir),
        };
        let cache_result = cached_file.get(&file_path);
        assert!(cache_result.is_ok());
        match cache_result.unwrap() {
            CacheResult::Miss(empty_cache_entry) => {
                let processed_file = process_file(&file_path, &configuration)?;
                let cache = configuration.get_cache();
                cache.write(&empty_cache_entry, &processed_file)?;
            }
            _ => {
                assert!(false)
            }
        }
        let cache_result = cached_file.get(&file_path);
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
