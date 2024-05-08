use std::path::PathBuf;

use crate::configuration;

#[derive(Debug)]
pub struct Reference<'a> {
    pub constant_definition: &'a ConstantDefinition,
    pub relative_referencing_file: String,
    pub referencing_source_location: SourceLocation,
}

#[derive(Debug, PartialEq, Clone)]
pub struct ConstantDefinition {
    pub fully_qualified_name: String,
    pub absolute_path_of_definition: PathBuf,
}
#[derive(Debug, PartialEq, Default, Eq, Clone)]
pub struct SourceLocation {
    line: usize,
    column: usize,
}

fn find<'a>(configuration: configuration::Configuration) -> anyhow::Result<Vec<Reference<'a>>> {
   todo!("Implement me!")
} 