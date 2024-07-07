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
        .try_fold(Vec::new, |mut acc, processed_file| {
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
        })
        .try_reduce(Vec::new, |mut acc, mut vec| {
            acc.append(&mut vec);
            Ok(acc)
        });
    references
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;
    use std::fs;

    use super::*;
    use crate::references::common_test::common_test::{configuration_for_fixture, SIMPLE_APP};
    use parser::SourceLocation;
    use pretty_assertions::assert_eq;

    fn expected_from_references_json(
        expected_references_json_path: &str,
    ) -> anyhow::Result<Vec<Reference>> {
        let file = std::fs::File::open(expected_references_json_path)?;
        let reader = std::io::BufReader::new(file);
        let expected: Vec<Reference> = serde_json::from_reader(reader)?;
        Ok(expected)
    }

    fn test_references(fixture_path: &str, expected: Vec<Reference>) -> anyhow::Result<()> {
        let configuration = configuration_for_fixture(fixture_path, true);
        let mut references = all_references(&configuration)?;
        references.sort();
        //let references_json = serde_json::to_string(&references)?;
        //std::fs::write(format!("{}/references.json", fixture_path), references_json)?;

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

    #[test]
    fn simple_all_references() -> anyhow::Result<()> {
        let expected = expected_from_references_json("tests/fixtures/simple_app/references.json")?;
        test_references(SIMPLE_APP, expected)
    }

    #[test]
    fn relationship_references() -> anyhow::Result<()> {
        let expected = expected_from_references_json(
            "tests/fixtures/app_with_rails_relationships/references.json",
        )?;
        test_references("tests/fixtures/app_with_rails_relationships", expected)
    }
}
