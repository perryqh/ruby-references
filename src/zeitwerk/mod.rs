mod constant_resolver;

use std::{
    collections::{HashMap, HashSet},
    path::{Path, PathBuf},
};

use tracing::debug;

use crate::{
    configuration::Configuration,
    constant_resolver::{ConstantDefinition, ConstantResolver},
    parser::inflector_shim,
};

use self::constant_resolver::ZeitwerkConstantResolver;

pub fn get_zeitwerk_constant_resolver(
    configuration: &Configuration,
) -> Box<dyn ConstantResolver + Send + Sync> {
    let constants = inferred_constants(configuration);

    ZeitwerkConstantResolver::create(constants)
}

fn inferred_constants(configuration: &Configuration) -> Vec<ConstantDefinition> {
    // First, we get a map of each autoload path to the files they map to.
    let autoload_paths_to_their_globbed_files = configuration
        .autoload_paths
        .keys()
        .map(|absolute_autoload_path| {
            let glob_path = absolute_autoload_path.join("**/*.rb");

            let files = glob::glob(glob_path.to_str().unwrap())
                .expect("Failed to read glob pattern")
                .filter_map(Result::ok)
                .collect::<Vec<PathBuf>>();

            (absolute_autoload_path, files)
        })
        .collect::<HashMap<&PathBuf, Vec<PathBuf>>>();

    debug!("Finding autoload path for each file");
    // Then, we want to know *which* autoload path is the one that defines a given constant.
    // The longest autoload path should be the one that does this.
    // For example, if we have two autoload paths:
    // 1) packs/my_pack/app/models
    // 2) packs/my_pack/app/models/concerns
    // And we have a file at `packs/my_pack/app/models/concerns/foo.rb`, we want to say that the constant `Foo` is defined by the second autoload path.
    // This is because the second autoload path is the longest path that contains the file.
    // We do this by creating a map of each file to the longest autoload path that contains it.
    let mut file_to_longest_path: HashMap<&PathBuf, &PathBuf> = HashMap::new();

    for (autoload_path, files) in &autoload_paths_to_their_globbed_files {
        for file in files {
            // Get the current longest path for this file, if it exists.
            let current_longest_path = file_to_longest_path
                .entry(file)
                .or_insert_with(|| autoload_path);

            // Update the longest path if the new path is longer.
            if autoload_path.components().count() > current_longest_path.components().count() {
                *current_longest_path = autoload_path;
            }
        }
    }

    debug!("Inferring constants from file name");
    let constants: Vec<ConstantDefinition> = file_to_longest_path
        .into_iter()
        .map(|(absolute_path_of_definition, absolute_autoload_path)| {
            let default_namespace = configuration
                .autoload_paths
                .get(absolute_autoload_path)
                .unwrap();
            inferred_constant_from_file(
                absolute_path_of_definition,
                absolute_autoload_path,
                &configuration.acronyms,
                default_namespace,
            )
        })
        .collect::<Vec<ConstantDefinition>>();

    constants
}

fn inferred_constant_from_file(
    absolute_path: &Path,
    absolute_autoload_path: &PathBuf,
    acronyms: &HashSet<String>,
    default_namespace: &String,
) -> ConstantDefinition {
    let relative_path = absolute_path.strip_prefix(absolute_autoload_path).unwrap();

    let relative_path = relative_path.with_extension("");

    let relative_path_str = relative_path.to_str().unwrap();
    let camelized_path = inflector_shim::camelize(relative_path_str, acronyms);
    let fully_qualified_name = format!("{}::{}", default_namespace, camelized_path);

    ConstantDefinition {
        fully_qualified_name,
        absolute_path_of_definition: absolute_path.to_path_buf(),
    }
}

#[cfg(test)]
mod tests {
    use crate::common_test::common_test::{
        get_absolute_root, get_zeitwerk_constant_resolver_for_fixture, SIMPLE_APP,
    };

    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn unnested_reference_to_unnested_constant() {
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo".to_string(),
                absolute_path_of_definition: get_absolute_root(SIMPLE_APP)
                    .join("packs/foo/app/services/foo.rb")
            }],
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP)
                .unwrap()
                .resolve(&String::from("Foo"), &[])
                .unwrap()
        );
    }

    #[test]
    fn constant_in_overridden_namespace() {
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Company::Widget".to_string(),
                absolute_path_of_definition: get_absolute_root(SIMPLE_APP)
                    .join("app/company_data/widget.rb")
            }],
            get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP)
                .unwrap()
                .resolve(&String::from("Widget"), &["Company"])
                .unwrap()
        );
    }

    #[test]
    fn nested_reference_to_unnested_constant() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver = get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo".to_string(),
                absolute_path_of_definition: absolute_root.join("packs/foo/app/services/foo.rb")
            }],
            resolver
                .resolve(&String::from("Foo"), &["Foo", "Bar", "Baz"])
                .unwrap()
        );
    }

    #[test]
    fn nested_reference_to_nested_constant() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver = get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo::Bar".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("packs/foo/app/services/foo/bar.rb")
            }],
            resolver.resolve("Bar", &["Foo"]).unwrap()
        );
    }

    #[test]
    fn nested_reference_to_global_constant() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver = get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Bar".to_string(),
                absolute_path_of_definition: absolute_root.join("packs/bar/app/services/bar.rb")
            }],
            resolver.resolve("::Bar", &["Foo"]).unwrap()
        );
    }

    #[test]
    fn nested_reference_to_constant_defined_within_another_file() {
        let absolute_root = get_absolute_root(SIMPLE_APP);
        let resolver = get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();
        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::Bar::BAR".to_string(),
                absolute_path_of_definition: absolute_root.join("packs/bar/app/services/bar.rb")
            }],
            resolver.resolve(&String::from("::Bar::BAR"), &[]).unwrap()
        );
    }

    #[test]
    fn inflected_constant() {
        let app = "tests/fixtures/app_with_inflections";
        let absolute_root = get_absolute_root(app);
        let resolver = get_zeitwerk_constant_resolver_for_fixture(app).unwrap();

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::MyModule::SomeAPIClass".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("app/services/my_module/some_api_class.rb")
            }],
            resolver
                .resolve(&String::from("::MyModule::SomeAPIClass"), &[])
                .unwrap()
        );

        assert_eq!(
            vec![ConstantDefinition {
                fully_qualified_name: "::MyModule::SomeCSVClass".to_string(),
                absolute_path_of_definition: absolute_root
                    .join("app/services/my_module/some_csv_class.rb")
            }],
            resolver
                .resolve(&String::from("::MyModule::SomeCSVClass"), &[])
                .unwrap()
        );
    }

    #[test]
    fn test_file_map() {
        let constant_resolver = get_zeitwerk_constant_resolver_for_fixture(SIMPLE_APP).unwrap();
        let actual_constant_map =
            constant_resolver.fully_qualified_constant_name_to_constant_definition_map();
        let absolute_root = get_absolute_root(SIMPLE_APP);

        let mut expected_constant_map = HashMap::new();
        expected_constant_map.insert(
            String::from("::Foo::Bar"),
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo::Bar".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/foo/app/services/foo/bar.rb"),
            }],
        );

        expected_constant_map.insert(
            "::Bar".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Bar".to_owned(),
                absolute_path_of_definition: absolute_root.join("packs/bar/app/services/bar.rb"),
            }],
        );
        expected_constant_map.insert(
            "::Baz".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Baz".to_owned(),
                absolute_path_of_definition: absolute_root.join("packs/baz/app/services/baz.rb"),
            }],
        );
        expected_constant_map.insert(
            "::Foo".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Foo".to_owned(),
                absolute_path_of_definition: absolute_root.join("packs/foo/app/services/foo.rb"),
            }],
        );
        expected_constant_map.insert(
            "::SomeConcern".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::SomeConcern".to_owned(),
                absolute_path_of_definition: absolute_root
                    .join("packs/bar/app/models/concerns/some_concern.rb"),
            }],
        );
        expected_constant_map.insert(
            "::SomeRootClass".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::SomeRootClass".to_owned(),
                absolute_path_of_definition: absolute_root.join("app/services/some_root_class.rb"),
            }],
        );
        expected_constant_map.insert(
            "::Company::Widget".to_owned(),
            vec![ConstantDefinition {
                fully_qualified_name: "::Company::Widget".to_owned(),
                absolute_path_of_definition: absolute_root.join("app/company_data/widget.rb"),
            }],
        );

        assert_eq!(&expected_constant_map, actual_constant_map);
    }
}
