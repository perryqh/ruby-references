use std::{
    collections::HashSet,
    path::{Path, PathBuf},
};

use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

use self::{file_utils::get_file_type, parser::process_from_path};

pub(crate) mod file_utils;
pub(crate) mod inflector_shim;
pub(crate) mod namespace_calculator;
pub(crate) mod parse_utils;
pub(crate) mod parser;
pub(crate) mod rails_utils;

#[derive(Debug, PartialEq)]
pub struct PreprocessedFile {
    pub absolute_path: PathBuf,
    pub unresolved_references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
}

#[derive(Debug, PartialEq)]
pub struct UnresolvedReference {
    pub name: String,
    pub namespace_path: Vec<String>,
    pub location: Range,
}

#[derive(Debug, PartialEq, Default, Clone, Copy)]
pub struct Range {
    pub start_row: usize,
    pub start_col: usize,
    pub end_row: usize,
    pub end_col: usize,
}

#[derive(Debug, PartialEq)]
pub struct ParsedDefinition {
    pub fully_qualified_name: String,
    pub location: Range,
}

pub(crate) fn preprocess(
    absolute_paths: &HashSet<PathBuf>,
) -> anyhow::Result<Vec<PreprocessedFile>> {
    absolute_paths
        .par_iter()
        .map(|absolute_path| -> anyhow::Result<PreprocessedFile> { process_file(absolute_path) })
        .collect()
}

pub fn process_file(path: &Path) -> anyhow::Result<PreprocessedFile> {
    let file_type_option = get_file_type(path);

    if let Some(file_type) = file_type_option {
        process_from_path(path)
    } else {
        // Later, we can perhaps have this error, since in theory the Configuration.intersect
        // method should make sure we never get any files we can't handle.
        Ok(PreprocessedFile {
            absolute_path: path.to_path_buf(),
            unresolved_references: vec![],
            definitions: vec![], // TODO
        })
    }
}
