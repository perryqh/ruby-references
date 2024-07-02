use std::{
    fs,
    path::{Path, PathBuf},
};

use anyhow::Context;
use lib_ruby_parser::{traverse::visitor::Visitor, Node, Parser, ParserOptions};
use line_col::LineColLookup;
use regex::Regex;

use crate::references::{configuration, parser::collector::ReferenceCollector};

use super::{self_reference_filterer, ProcessedFile};

pub fn process_file(
    path: &PathBuf,
    configuration: &configuration::Configuration,
) -> anyhow::Result<ProcessedFile> {
    let contents = match get_file_type(path, configuration) {
        Some(SupportedFileType::Ruby) => file_read_contents(path)?,
        Some(SupportedFileType::Erb) => {
            let c = file_read_contents(path)?;
            convert_erb_to_ruby_without_sourcemaps(c)
        }
        None => {
            return Ok(ProcessedFile {
                absolute_path: path.to_path_buf(),
                ..Default::default()
            })
        }
    };

    process_from_contents(contents, path, configuration)
}

#[derive(PartialEq, Debug)]
enum SupportedFileType {
    Ruby,
    Erb,
}

fn get_file_type(
    path: &Path,
    configuration: &configuration::Configuration,
) -> Option<SupportedFileType> {
    let extension = path.extension();

    if extension.map_or(false, |ext| ext == "erb") {
        return Some(SupportedFileType::Erb);
    }

    let is_ruby_file = configuration
        .ruby_extensions
        .iter()
        .any(|ext| extension.map_or(false, |e| e == *ext))
        || configuration
            .ruby_special_files
            .iter()
            .any(|file| path.ends_with(file));

    if is_ruby_file {
        Some(SupportedFileType::Ruby)
    } else {
        None
    }
}

const ERB_REGEX: &str = r"(?s)<%=?-?\s*(.*?)\s*-?%>";

fn convert_erb_to_ruby_without_sourcemaps(contents: String) -> String {
    let regex = Regex::new(ERB_REGEX).unwrap();

    let extracted_contents: Vec<&str> = regex
        .captures_iter(&contents)
        .map(|capture| capture.get(1).unwrap().as_str())
        .collect();

    extracted_contents.join("\n")
}

fn file_read_contents(path: &PathBuf) -> anyhow::Result<String> {
    fs::read_to_string(path).context(format!(
        "Failed to read contents of {}",
        path.to_string_lossy()
    ))
}

fn process_from_contents(
    contents: String,
    path: &PathBuf,
    configuration: &configuration::Configuration,
) -> anyhow::Result<ProcessedFile> {
    let lookup = LineColLookup::new(&contents);

    let ast = match build_ast(contents.clone()) {
        Some(ast) => ast,
        None => {
            return Ok(ProcessedFile {
                absolute_path: path.clone(),
                unresolved_references: vec![],
            })
        }
    };

    let mut collector = ReferenceCollector::new(lookup, configuration.custom_associations.clone());

    collector.visit(&ast);

    let unresolved_references = if configuration.include_reference_is_definition {
        collector.references
    } else {
        self_reference_filterer::filter(collector)
    };

    Ok(ProcessedFile {
        absolute_path: path.to_owned(),
        unresolved_references,
    })
}

fn build_ast(contents: String) -> Option<Box<Node>> {
    let options = ParserOptions {
        buffer_name: "".to_string(),
        ..Default::default()
    };
    let parser = Parser::new(contents, options);
    let parse_result = parser.do_parse();
    parse_result.ast
}

#[cfg(test)]
mod tests {
    use crate::references::configuration::Configuration;

    use super::*;

    fn process(path: &str, include_self_references: bool) -> ProcessedFile {
        let path = PathBuf::from(path);
        let configuration = Configuration {
            include_reference_is_definition: include_self_references,
            ..Default::default()
        };
        process_file(&path, &configuration).unwrap()
    }
    #[test]
    fn process_from_path() {
        let path = "tests/fixtures/small-app/app/models/client_invitation.rb";
        let processed_file = process(path, false);
        assert_eq!(processed_file.absolute_path.to_str(), Some(path));
        assert_eq!(processed_file.unresolved_references.len(), 8);
        let reference_names: Vec<&str> = processed_file
            .unresolved_references
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(
            reference_names,
            vec![
                "ApplicationRecord",
                "::ClientInvitation",
                "HasUuid",
                "AccountingFirm",
                "T::Enum",
                "::ClientInvitation::InvitationType",
                "T::Enum",
                "::ClientInvitation::InvitationTrigger"
            ]
        );
    }

    #[test]
    fn process_from_path_with_self_references() {
        let path = "tests/fixtures/small-app/app/models/client_invitation.rb";
        let processed_file = process(path, true);
        assert_eq!(processed_file.absolute_path.to_str(), Some(path));
        assert_eq!(processed_file.unresolved_references.len(), 9);
        let reference_names: Vec<&str> = processed_file
            .unresolved_references
            .iter()
            .map(|r| r.name.as_str())
            .collect();
        assert_eq!(
            reference_names,
            vec![
                "ApplicationRecord",
                "::ClientInvitation",
                "HasUuid",
                "AccountingFirm",
                "T::Enum",
                "::ClientInvitation::InvitationType",
                "T::Enum",
                "::ClientInvitation::InvitationTrigger",
                "InvitationType"
            ]
        );
    }

    #[test]
    fn process_erb_file() {
        let processed_file = process(
            "tests/fixtures/small-app/app/views/layouts/application.html.erb",
            false,
        );
        assert_eq!(processed_file.unresolved_references.len(), 1);
        assert_eq!(processed_file.unresolved_references[0].name, "Admin::User");
    }

    #[test]
    fn file_type() {
        let test_mapping = vec![
            (
                "tests/fixtures/small-app/app/models/client_invitation.rb",
                Some(SupportedFileType::Ruby),
            ),
            (
                "tests/fixtures/small-app/my.rake",
                Some(SupportedFileType::Ruby),
            ),
            (
                "tests/fixtures/small-app/my.builder",
                Some(SupportedFileType::Ruby),
            ),
            (
                "tests/fixtures/small-app/my.gemspec",
                Some(SupportedFileType::Ruby),
            ),
            (
                "tests/fixtures/small-app/my.ru",
                Some(SupportedFileType::Ruby),
            ),
            (
                "tests/fixtures/small-app/app/views/layouts/application.html.erb",
                Some(SupportedFileType::Erb),
            ),
            (
                "tests/fixtures/small-app/Gemfile",
                Some(SupportedFileType::Ruby),
            ),
            (
                "tests/fixtures/small-app/Rakefile",
                Some(SupportedFileType::Ruby),
            ),
            ("tests/fixtures/my.rs", None),
        ];
        let configuration = Configuration::default();
        for (path, expected) in test_mapping {
            let path = PathBuf::from(path);
            assert_eq!(get_file_type(&path, &configuration), expected);
        }
    }
}
