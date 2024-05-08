use std::{collections::HashMap, path::PathBuf};

#[derive(Debug)]
pub struct Configuration<'a> {
    pub absolute_root: &'a PathBuf,
    pub inflections_path: &'a PathBuf,
    pub autoload_roots: &'a HashMap<PathBuf, String>,
}
