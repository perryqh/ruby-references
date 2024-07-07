use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use anyhow::Context;
use serde::{Deserialize, Serialize};

use crate::references::{
    configuration::Configuration,
    constant_resolver::{ConstantDefinition, ConstantResolver},
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
        ReferencesBuilder::default()
            .configuration(configuration)
            .constant_resolver(constant_resolver)
            .referencing_file_path(referencing_file_path)?
            .unresolved_reference(unresolved_reference)?
            .build()
    }
}
#[derive(Default)]
struct ReferencesBuilder<'a> {
    configuration: Option<&'a Configuration>,
    referencing_file_path: Option<PathBuf>,
    relative_referencing_file: String,
    source_location: Option<SourceLocation>,
    constant_resolver: Option<&'a (dyn ConstantResolver + Send + Sync)>,
    constant_definition: Option<Vec<ConstantDefinition>>,
    unresolved_reference_name: Option<String>,
}

impl<'a> ReferencesBuilder<'a> {
    fn build(self) -> anyhow::Result<Vec<Reference>> {
        if let Some(constant_definitions) = self.constant_definition.clone() {
            self.references_from_constant_definitions(constant_definitions)
        } else {
            self.references_without_constant_defintions()
        }
    }

    fn references_without_constant_defintions(self) -> anyhow::Result<Vec<Reference>> {
        let relative_defining_file = None;
        // Contant name is not known, so we'll just use the unresolved name for now
        let constant_name = self
            .unresolved_reference_name
            .context("expected unresolved_reference_name")?
            .clone();
        let extra_fields = self
            .configuration
            .context("expecting configuration")?
            .extra_reference_fields_fn
            .as_ref()
            .map(|fn_| {
                fn_.extra_reference_fields_fn(
                    &self
                        .referencing_file_path
                        .expect("expecting relative_referencing_file_path"),
                    None,
                )
            })
            .unwrap_or_default();
        Ok(vec![Reference {
            constant_name,
            relative_referencing_file: self.relative_referencing_file,
            source_location: self.source_location.context("expecting source_location")?,
            relative_defining_file,
            extra_fields,
        }])
    }

    fn references_from_constant_definitions(
        self,
        constant_definitions: Vec<ConstantDefinition>,
    ) -> anyhow::Result<Vec<Reference>> {
        let absolute_root = self
            .configuration
            .context("expecting configuration")?
            .absolute_root
            .clone();
        Ok(constant_definitions
                .iter()
                .map(move |constant| {
                    let absolute_path_of_definition = &constant.absolute_path_of_definition;
                    let relative_defining_file = absolute_path_of_definition
                        .strip_prefix(&absolute_root)
                        .context(format!("expecting strip_prefix. absolute_path_of_definition: {:?}, absolute_root: {:?}", &absolute_path_of_definition, &absolute_root))?
                        .to_path_buf()
                        .to_str()
                        .context("expecting relative_defining_file")?
                        .to_string();

                    let relative_defining_file = Some(relative_defining_file);
                    let constant_name = constant.fully_qualified_name.clone();
                    let extra_fields = self.configuration
                        .context("expecting configuration")?
                        .extra_reference_fields_fn
                        .as_ref()
                        .map(|fn_| fn_.extra_reference_fields_fn(&self.referencing_file_path.clone().expect("expecting referencing_file_path"), Some(absolute_path_of_definition)))
                        .unwrap_or_default();

                    Ok(Reference {
                        constant_name,
                        relative_referencing_file: self.relative_referencing_file.clone(),
                        source_location: self.source_location.clone().context("expecting source_location")?.clone(),
                        relative_defining_file,
                        extra_fields,
                    })
                })
                .collect::<anyhow::Result<Vec<Reference>>>()?)
    }

    fn configuration(mut self, configuration: &'a Configuration) -> Self {
        self.configuration = Some(configuration);
        self
    }

    fn referencing_file_path(mut self, referencing_file_path: &Path) -> anyhow::Result<Self> {
        let absolute_root = self
            .configuration
            .context("expecting configuration")?
            .absolute_root
            .clone();

        let relative_referencing_file_path = referencing_file_path
            .strip_prefix(&absolute_root)
            .context(format!(
                "expecting strip_prefix. referencing_file_path: {:?}, absolute_root: {:?}",
                &referencing_file_path, absolute_root
            ))?
            .to_path_buf();
        self.relative_referencing_file = relative_referencing_file_path
            .to_str()
            .context("expecting relative_referencing_file_path to allow to_str")?
            .to_string();
        self.referencing_file_path = Some(referencing_file_path.to_path_buf());
        Ok(self)
    }

    fn unresolved_reference(
        mut self,
        unresolved_reference: &UnresolvedReference,
    ) -> anyhow::Result<Self> {
        let loc = &unresolved_reference.location;
        self.source_location = Some(SourceLocation {
            line: loc.start_row,
            column: loc.start_col,
        });

        let str_namespace_path: Vec<&str> = unresolved_reference
            .namespace_path
            .iter()
            .map(|s| s.as_str())
            .collect::<Vec<&str>>();

        self.constant_definition = self
            .constant_resolver
            .context("expecting constant_resolver")?
            .resolve(&unresolved_reference.name, &str_namespace_path);
        self.unresolved_reference_name = Some(unresolved_reference.name.clone());

        Ok(self)
    }

    fn constant_resolver(
        mut self,
        constant_resolver: &'a (dyn ConstantResolver + Send + Sync),
    ) -> Self {
        self.constant_resolver = Some(constant_resolver);
        self
    }
}
