use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
};

#[derive(Debug)]
pub struct Configuration {
    pub absolute_root: PathBuf,
    pub included_files: HashSet<PathBuf>,
    pub acronyms: HashSet<String>,
    // has pack.default_autoload_roots and pack.autoload_roots
    pub autoload_paths: HashMap<PathBuf, String>,
    pub custom_associations: Vec<String>,
    pub ruby_special_files: Vec<&'static str>,
    pub ruby_extensions: Vec<&'static str>,
    // Include references whose constants are defined in the same file
    pub include_reference_is_definition: bool,
}

impl Default for Configuration {
    fn default() -> Self {
        Configuration {
            absolute_root: PathBuf::from(""),
            included_files: HashSet::new(),
            acronyms: HashSet::new(),
            autoload_paths: HashMap::new(),
            custom_associations: Vec::new(),
            ruby_special_files: vec!["Gemfile", "Rakefile"],
            ruby_extensions: vec!["rb", "rake", "builder", "gemspec", "ru"],
            include_reference_is_definition: false,
        }
    }
}
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_configuration() {
        let configuration = Configuration::default();
        assert_eq!(configuration.absolute_root, PathBuf::from(""));
        assert_eq!(configuration.acronyms, HashSet::new());
        assert_eq!(configuration.autoload_paths, HashMap::new());
        assert_eq!(configuration.custom_associations, Vec::<String>::new());
        assert_eq!(
            configuration.ruby_special_files,
            vec!["Gemfile", "Rakefile"]
        );
        assert_eq!(
            configuration.ruby_extensions,
            vec!["rb", "rake", "builder", "gemspec", "ru"]
        );
    }
}
