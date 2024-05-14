#[cfg(test)]
pub mod common_test {
    use std::{
        collections::{HashMap, HashSet},
        path::PathBuf,
    };

    use regex::Regex;
    use walkdir::WalkDir;
    use yaml_rust::YamlLoader;

    use crate::{
        configuration::Configuration, constant_resolver::ConstantResolver,
        zeitwerk::get_zeitwerk_constant_resolver,
    };

    pub fn configuration_for_fixture(fixture_name: &str) -> Configuration {
        let absolute_root = get_absolute_root(fixture_name);
        let autoload_paths = autoload_paths_for_fixture(&absolute_root).unwrap();
        let acronyms = acronyms(&absolute_root);
        let included_files = file_paths(fixture_name).unwrap();
        Configuration {
            absolute_root,
            autoload_paths,
            acronyms,
            included_files,
            include_reference_is_definition: false,
            ..Default::default()
        }
    }

    pub fn get_zeitwerk_constant_resolver_for_fixture(
        fixture_name: &str,
    ) -> anyhow::Result<Box<dyn ConstantResolver>> {
        let configuration = configuration_for_fixture(fixture_name);

        Ok(get_zeitwerk_constant_resolver(&configuration))
    }

    fn acronyms(root: &PathBuf) -> HashSet<String> {
        let mut acronyms = HashSet::new();
        let inflections_path = root.join("config/initializers/inflections.rb");
        if inflections_path.exists() {
            let inflections_file = std::fs::read_to_string(inflections_path).unwrap();
            let inflections_lines = inflections_file.lines();
            for line in inflections_lines {
                if line.contains(".acronym") {
                    let re = Regex::new(r#"['\\"]"#).unwrap();
                    let acronym = re.split(line).nth(1).unwrap();
                    acronyms.insert(acronym.to_string());
                }
            }
        }
        acronyms
    }

    fn extract_autoload_paths_from_packwerk_config(
        root: &PathBuf,
    ) -> anyhow::Result<HashMap<PathBuf, String>> {
        let mut extra = HashMap::new();
        let packwerk_config_path = root.join("packwerk.yml");
        match std::fs::read_to_string(packwerk_config_path) {
            Ok(packwerk_config_str) => {
                let p_yaml = YamlLoader::load_from_str(&packwerk_config_str)?
                    .pop()
                    .unwrap();
                match &p_yaml["autoload_roots"] {
                    yaml_rust::Yaml::Hash(autoload_roots) => {
                        for (path, value) in autoload_roots {
                            let abs_path = root.join(path.as_str().unwrap());
                            let value_str = value.as_str().unwrap();
                            extra.insert(abs_path, String::from(value_str));
                        }
                    }
                    _ => {}
                }
            }
            Err(e) => println!("{:?}", e),
        }
        Ok(extra)
    }

    fn autoload_paths_for_fixture(root: &PathBuf) -> anyhow::Result<HashMap<PathBuf, String>> {
        let mut full_autoload_roots: HashMap<PathBuf, String> = HashMap::new();

        for entry in glob::glob(root.join("**/package.yml").as_path().to_str().unwrap())? {
            match entry {
                Ok(path) => {
                    let root_pattern = path.parent().unwrap().join("app").join("*");
                    let concerns_pattern = root_pattern.join("concerns");
                    let mut roots = expand_glob(root_pattern.to_str().unwrap());
                    roots.extend(expand_glob(concerns_pattern.to_str().unwrap()));
                    for root in roots {
                        full_autoload_roots.insert(root, String::from(""));
                    }
                }
                Err(e) => println!("{:?}", e),
            }
        }
        for (path, value) in extract_autoload_paths_from_packwerk_config(root)? {
            full_autoload_roots.insert(path, value);
        }

        Ok(full_autoload_roots)
    }

    fn expand_glob(pattern: &str) -> Vec<PathBuf> {
        glob::glob(pattern).unwrap().map(|p| p.unwrap()).collect()
    }

    pub const SIMPLE_APP: &str = "tests/fixtures/simple_app";

    pub fn get_absolute_root(fixture_name: &str) -> PathBuf {
        PathBuf::from(fixture_name).canonicalize().unwrap()
    }

    pub fn file_paths(root: &str) -> anyhow::Result<HashSet<PathBuf>> {
        let paths = WalkDir::new(root)
            .into_iter()
            .filter_map(|entry| entry.ok())
            .filter_map(|entry| {
                if entry.file_type().is_file()
                    && entry.path().extension().map_or(false, |ext| ext == "rb")
                    && !entry.path().to_str().unwrap().contains("node_modules")
                {
                    Some(entry.path().canonicalize().unwrap().to_path_buf())
                } else {
                    None
                }
            })
            .collect::<std::collections::HashSet<PathBuf>>();
        Ok(paths)
    }
}
