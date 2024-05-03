use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Debug)]
pub struct Reference {
    pub constant_name: String,
    pub relative_defining_file: Option<String>,
    pub relative_referencing_file: String,
    pub referencing_source_location: SourceLocation,
}

#[derive(Debug)]
pub struct ReferenceConfig<'a> {
    pub absolute_root: &'a PathBuf,
    pub inflections_path: &'a PathBuf,
    pub autoload_roots: &'a HashMap<PathBuf, String>,
}

#[derive(Debug, PartialEq, Default, Eq, Clone)]
pub struct SourceLocation {
    line: usize,
    column: usize,
}

pub(crate) fn build_references(
    configuration: &ReferenceConfig,
    absolute_paths: &HashSet<PathBuf>,
) -> anyhow::Result<Vec<Reference>> {
    unimplemented!("Implement me!")
}

// Thinking about keeping the concept of packs out of here as ruby definitions and references
// can be of use to other libraries. Also, the concept of packs is fluid.
// Potential complications:
// - Caching. Should we leave caching up consumers of this crate?
//   - for the case of packs, there will likely be a PackReference struct that can be cached
// - We'd need to iterate over the files again to decorate the reference with PackReference
