use std::path::PathBuf;

use crate::configuration;

use self::processor::process_file;

pub(crate) mod collector;
pub(crate) mod inflector_shim;
pub(crate) mod namespace_calculator;
pub(crate) mod processor;
pub(crate) mod self_reference_filterer;

#[derive(Debug, PartialEq, Eq, Clone, Default)]
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

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ProcessedFile {
    pub absolute_path: PathBuf,
    pub unresolved_references: Vec<UnresolvedReference>,
}

#[derive(Debug, PartialEq, Default, Eq, Clone, PartialOrd, Ord)]
pub struct SourceLocation {
    pub line: usize,
    pub column: usize,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub struct UnresolvedReference {
    pub name: String,
    pub namespace_path: Vec<String>,
    pub location: Range,
}

pub fn parse(configuration: &configuration::Configuration) -> anyhow::Result<Vec<ProcessedFile>> {
    configuration
        .included_files
        .iter()
        .map(|path| process_file(path, configuration))
        .collect()
}

#[cfg(test)]
mod tests {
    use crate::common_test::common_test::file_paths;

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
}
