pub(crate) mod cache;
pub(crate) mod cached_file;
pub mod configuration;
pub(crate) mod constant_resolver;
pub(crate) mod parser;
pub mod reference;
pub(crate) mod zeitwerk;

pub(crate) mod common_test;

use crate::references::configuration::Configuration;
use crate::references::parser::parse;
use crate::references::reference::Reference;
use crate::references::zeitwerk::get_zeitwerk_constant_resolver;

use anyhow::Context;
use rayon::iter::{IntoParallelRefIterator, ParallelIterator};

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
    use crate::references::parser::SourceLocation;
    use std::collections::HashMap;
    use std::fs;

    use super::*;
    use crate::references::common_test::common_test::{configuration_for_fixture, SIMPLE_APP};
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
