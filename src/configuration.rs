use std::{
    collections::{HashMap, HashSet},
    fmt,
    path::PathBuf,
};

use crate::{
    cache::{create_cache_dir_idempotently, Cache, NoopCache},
    cached_file::CachedFile,
};

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
    pub cache_enabled: bool,
    pub cache_directory: PathBuf,
    pub extra_reference_fields_fn: Option<Box<dyn ExtraReferenceFieldsFn>>,
}

pub trait ExtraReferenceFieldsFn: Sync + Send {
    fn extra_reference_fields_fn(
        &self,
        relative_referencing_file: &str,
        relative_defining_file: Option<&str>,
    ) -> HashMap<String, String>;
}

impl fmt::Debug for Configuration {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Configuration")
            .field("absolute_root", &self.absolute_root)
            .field("included_files", &self.included_files)
            .field("acronyms", &self.acronyms)
            .field("autoload_paths", &self.autoload_paths)
            .field("custom_associations", &self.custom_associations)
            .field("ruby_special_files", &self.ruby_special_files)
            .field("ruby_extensions", &self.ruby_extensions)
            .field("cache_enabled", &self.cache_enabled)
            .field("cache_directory", &self.cache_directory)
            .field(
                "include_reference_is_definition",
                &self.include_reference_is_definition,
            )
            // Skip `extra_reference_fields` because it cannot be formatted using Debug
            .finish()
    }
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
            cache_enabled: false,
            cache_directory: PathBuf::from("tmp/cache"),
            extra_reference_fields_fn: None,
        }
    }
}

impl Configuration {
    pub(crate) fn get_cache(&self) -> Box<dyn Cache + Send + Sync> {
        if self.cache_enabled {
            let cache_dir = self.cache_directory.join("ruby-references");

            let _ = create_cache_dir_idempotently(&cache_dir);

            Box::new(CachedFile { cache_dir })
        } else {
            Box::new(NoopCache {})
        }
    }

    pub(crate) fn delete_cache(&self) -> anyhow::Result<()> {
        Ok(std::fs::remove_dir_all(&self.cache_directory)?)
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
