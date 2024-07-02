use std::{collections::HashMap, path::Path};

use anyhow::Context;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};
use serde::{Deserialize, Serialize};

use crate::{
    configuration::Configuration,
    constant_resolver::ConstantResolver,
    parser::{parse, SourceLocation, UnresolvedReference},
    zeitwerk::get_zeitwerk_constant_resolver,
};

#[derive(Debug, PartialEq, Eq, Clone, Serialize, Deserialize)]
pub struct Reference {
    pub constant_name: String,
    pub relative_defining_file: Option<String>,
    pub relative_referencing_file: String,
    pub source_location: SourceLocation,
    pub extra_fields: HashMap<String, String>,
}

impl Ord for Reference {
    fn cmp(&self, other: &Self) -> std::cmp::Ordering {
        self.constant_name
            .cmp(&other.constant_name)
            .then_with(|| {
                self.relative_defining_file
                    .as_ref()
                    .cmp(&other.relative_defining_file.as_ref())
            })
            .then_with(|| {
                self.relative_referencing_file
                    .cmp(&other.relative_referencing_file)
            })
            .then_with(|| self.source_location.line.cmp(&other.source_location.line))
            .then_with(|| {
                self.source_location
                    .column
                    .cmp(&other.source_location.column)
            })
            .then_with(|| self.extra_fields.len().cmp(&other.extra_fields.len()))
    }
}

impl PartialOrd for Reference {
    fn partial_cmp(&self, other: &Self) -> Option<std::cmp::Ordering> {
        Some(self.cmp(other))
    }
}

impl Reference {
    pub fn from_unresolved_reference(
        configuration: &Configuration,
        constant_resolver: &(dyn ConstantResolver + Send + Sync),
        unresolved_reference: &UnresolvedReference,
        referencing_file_path: &Path,
    ) -> anyhow::Result<Vec<Reference>> {
        let loc = &unresolved_reference.location;
        let source_location = SourceLocation {
            line: loc.start_row,
            column: loc.start_col,
        };
        let relative_referencing_file_path = referencing_file_path
            .strip_prefix(&configuration.absolute_root)
            .context(format!(
                "expecting strip_prefix. referencing_file_path: {:?}, absolute_root: {:?}",
                &referencing_file_path, &configuration.absolute_root
            ))?
            .to_path_buf();
        let relative_referencing_file = relative_referencing_file_path
            .to_str()
            .context("expecting relative_referencing_file_path")?
            .to_string();
        let str_namespace_path: Vec<&str> = unresolved_reference
            .namespace_path
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>();
        let maybe_constant_definition =
            constant_resolver.resolve(&unresolved_reference.name, &str_namespace_path);

        if let Some(constant_definitions) = &maybe_constant_definition {
            Ok(constant_definitions
                .iter()
                .map(move |constant| {
                    let absolute_path_of_definition = &constant.absolute_path_of_definition;
                    let relative_defining_file = absolute_path_of_definition
                        .strip_prefix(&configuration.absolute_root)
                        .context(format!("expecting strip_prefix. absolute_path_of_definition: {:?}, absolute_root: {:?}", &absolute_path_of_definition, &configuration.absolute_root))?
                        .to_path_buf()
                        .to_str()
                        .context("expecting relative_defining_file")?
                        .to_string();

                    let relative_defining_file = Some(relative_defining_file);
                    let constant_name = constant.fully_qualified_name.clone();
                    let extra_fields = configuration
                        .extra_reference_fields_fn
                        .as_ref()
                        .map(|fn_| fn_.extra_reference_fields_fn(&referencing_file_path.to_path_buf(), Some(absolute_path_of_definition)))
                        .unwrap_or_default();

                    Ok(Reference {
                        constant_name,
                        relative_referencing_file: relative_referencing_file.clone(),
                        source_location: source_location.clone(),
                        relative_defining_file,
                        extra_fields,
                    })
                })
                .collect::<anyhow::Result<Vec<Reference>>>()?)
        } else {
            let relative_defining_file = None;
            // Contant name is not known, so we'll just use the unresolved name for now
            let constant_name = unresolved_reference.name.clone();
            let extra_fields = configuration
                .extra_reference_fields_fn
                .as_ref()
                .map(|fn_| {
                    fn_.extra_reference_fields_fn(&referencing_file_path.to_path_buf(), None)
                })
                .unwrap_or_default();

            Ok(vec![Reference {
                constant_name,
                relative_referencing_file,
                source_location,
                relative_defining_file,
                extra_fields,
            }])
        }
    }
}

pub fn all_references(configuration: &Configuration) -> anyhow::Result<Vec<Reference>> {
    let processed_files_to_check =
        parse(configuration).context("failed to parse processed files")?;
    let constant_resolver = get_zeitwerk_constant_resolver(configuration);
    let references: anyhow::Result<Vec<Reference>> = processed_files_to_check
        .par_iter()
        .try_fold(
            Vec::new,
            // Start with an empty vector for each thread
            |mut acc, processed_file| {
                for unresolved_ref in processed_file.unresolved_references.iter() {
                    let new_references = Reference::from_unresolved_reference(
                        configuration,
                        constant_resolver.as_ref(),
                        unresolved_ref,
                        &processed_file.absolute_path,
                    )?;
                    acc.extend(new_references);
                }
                Ok(acc)
            },
        )
        .try_reduce(Vec::new, |mut acc, mut vec| {
            acc.append(&mut vec);
            Ok(acc)
        });
    references
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::*;
    use crate::common_test::common_test::{configuration_for_fixture, SIMPLE_APP};
    use pretty_assertions::assert_eq;

    #[test]
    fn simple_all_references() -> anyhow::Result<()> {
        let configuration = configuration_for_fixture(SIMPLE_APP, true);
        let mut references = all_references(&configuration)?;
        references.sort();
        let expected = json::parse(&fs::read_to_string(
            "tests/fixtures/simple_app/references.json",
        )?)?;
        let mut expected = expected
            .members()
            .map(|m| {
                let mut extra_fields = HashMap::new();
                extra_fields.insert(
                    "referencing_pack_name".to_string(),
                    m["referencing_pack_name"].as_str().unwrap().to_string(),
                );
                if let Some(defining_pack_name) = m["defining_pack_name"].as_str() {
                    extra_fields.insert(
                        "defining_pack_name".to_string(),
                        defining_pack_name.to_string(),
                    );
                }
                Reference {
                    constant_name: m["constant_name"].as_str().unwrap().to_string(),
                    relative_referencing_file: m["relative_referencing_file"]
                        .as_str()
                        .unwrap()
                        .to_string(),
                    relative_defining_file: m["relative_defining_file"]
                        .as_str()
                        .map(|s| s.to_string()),
                    source_location: SourceLocation {
                        line: m["source_location"]["line"].as_usize().unwrap(),
                        column: m["source_location"]["column"].as_usize().unwrap(),
                    },
                    extra_fields,
                }
            })
            .collect::<Vec<Reference>>();
        expected.sort();
        assert_eq!(references.len(), expected.len());
        for (reference, expected) in references.iter().zip(expected.iter()) {
            assert_eq!(reference, expected);
        }

        let mut cache_hit_references = all_references(&configuration)?;
        cache_hit_references.sort();
        for (reference, expected) in cache_hit_references.iter().zip(expected.iter()) {
            assert_eq!(reference, expected);
        }
        configuration.delete_cache()?;
        Ok(())
    }
}
