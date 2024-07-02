use std::{collections::HashMap, path::Path};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::references::{
    configuration::Configuration,
    constant_resolver::ConstantResolver,
    parser::{SourceLocation, UnresolvedReference},
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
