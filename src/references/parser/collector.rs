use lib_ruby_parser::{nodes, traverse::visitor::Visitor, Loc, Node};
use line_col::LineColLookup;
use std::collections::HashSet;

use super::{inflector_shim::to_class_case, ParsedDefinition, Range, UnresolvedReference};

#[derive(Debug)]
pub enum ParseError {
    Metaprogramming,
}

#[derive(Debug, PartialEq, Eq)]
pub struct SuperclassReference {
    pub name: String,
    pub namespace_path: Vec<String>,
}

pub struct ReferenceCollector<'a> {
    pub references: Vec<UnresolvedReference>,
    pub definitions: Vec<ParsedDefinition>,
    pub current_namespaces: Vec<String>,
    pub line_col_lookup: LineColLookup<'a>,
    pub in_superclass: bool,
    pub superclasses: Vec<SuperclassReference>,
    pub custom_associations: Vec<String>,
}

impl<'a> ReferenceCollector<'a> {
    pub fn new(line_col_lookup: LineColLookup<'a>, custom_associations: Vec<String>) -> Self {
        ReferenceCollector {
            references: vec![],
            definitions: vec![],
            current_namespaces: vec![],
            line_col_lookup,
            in_superclass: false,
            superclasses: vec![],
            custom_associations,
        }
    }
}

const ASSOCIATION_METHOD_NAMES: [&str; 4] = [
    "has_one",
    "has_many",
    "belongs_to",
    "has_and_belongs_to_many",
];

impl<'a> Visitor for ReferenceCollector<'a> {
    fn on_class(&mut self, node: &nodes::Class) {
        // We're not collecting definitions, so no need to visit the class definitioname);
        let namespace_result = fetch_const_name(&node.name);
        // For now, we simply exit and stop traversing if we encounter an error when fetching the constant name of a class
        // We can iterate on this if this is different than the packwerk implementation
        if namespace_result.is_err() {
            return;
        }

        let namespace = namespace_result.unwrap();

        if let Some(inner) = node.superclass.as_ref() {
            self.in_superclass = true;
            self.visit(inner);
            self.in_superclass = false;
        }
        let definition_loc = fetch_node_location(&node.name).unwrap();
        let location = loc_to_range(definition_loc, &self.line_col_lookup);

        let definition = get_definition_from(&namespace, &self.current_namespaces, &location);

        let name = definition.fully_qualified_name.to_owned();
        let namespace_path = self.current_namespaces.to_owned();
        self.definitions.push(definition);

        // Packwerk also considers a definition to be a "reference"
        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location,
        });

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        if let Some(inner) = &node.body {
            self.visit(inner);
        }

        self.current_namespaces.pop();
        self.superclasses.pop();
    }

    fn on_send(&mut self, node: &nodes::Send) {
        let association_reference = get_reference_from_active_record_association(
            node,
            &self.current_namespaces,
            &self.line_col_lookup,
            &self.custom_associations,
        );

        if let Some(association_reference) = association_reference {
            self.references.push(association_reference);
        }

        lib_ruby_parser::traverse::visitor::visit_send(self, node);
    }

    fn on_casgn(&mut self, node: &nodes::Casgn) {
        let definition = get_constant_assignment_definition(
            node,
            self.current_namespaces.to_owned(),
            &self.line_col_lookup,
        );

        if let Some(definition) = definition {
            self.definitions.push(definition);
        }

        if let Some(v) = node.value.to_owned() {
            self.visit(&v);
        } else {
            // We don't handle constant assignments as part of a multi-assignment yet,
            // e.g. A, B = 1, 2
            // See the documentation for nodes::Casgn#value for more info.
        }
    }

    fn on_module(&mut self, node: &nodes::Module) {
        let namespace = fetch_const_name(&node.name)
            .expect("We expect no parse errors in class/module definitions");
        let definition_loc = fetch_node_location(&node.name).unwrap();
        let location = loc_to_range(definition_loc, &self.line_col_lookup);

        let definition = get_definition_from(&namespace, &self.current_namespaces, &location);

        let name = definition.fully_qualified_name.to_owned();
        let namespace_path = self.current_namespaces.to_owned();
        self.definitions.push(definition);

        // Packwerk also considers a definition to be a "reference"
        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location,
        });

        // Note – is there a way to use lifetime specifiers to get rid of this and
        // just keep current namespaces as a vector of string references or something else
        // more efficient?
        self.current_namespaces.push(namespace);

        if let Some(inner) = &node.body {
            self.visit(inner);
        }

        self.current_namespaces.pop();
    }

    fn on_const(&mut self, node: &nodes::Const) {
        let Ok(name) = fetch_const_const_name(node) else {
            return;
        };

        if self.in_superclass {
            self.superclasses.push(SuperclassReference {
                name: name.to_owned(),
                namespace_path: self.current_namespaces.to_owned(),
            })
        }
        // In packwerk, NodeHelpers.enclosing_namespace_path ignores
        // namespaces where a superclass OR namespace is the same as the current reference name
        let matching_superclass_option = self
            .superclasses
            .iter()
            .find(|superclass| superclass.name == name);

        let namespace_path = if let Some(matching_superclass) = matching_superclass_option {
            matching_superclass.namespace_path.to_owned()
        } else {
            self.current_namespaces
                .clone()
                .into_iter()
                .filter(|namespace| {
                    namespace != &name
                        || self
                            .superclasses
                            .iter()
                            .any(|superclass| superclass.name == name)
                })
                .collect::<Vec<String>>()
        };

        self.references.push(UnresolvedReference {
            name,
            namespace_path,
            location: loc_to_range(&node.expression_l, &self.line_col_lookup),
        })
    }
}

fn fetch_const_name(node: &nodes::Node) -> Result<String, ParseError> {
    match node {
        Node::Const(const_node) => Ok(fetch_const_const_name(const_node)?),
        Node::Cbase(_) => Ok(String::from("")),
        Node::Send(_) => Err(ParseError::Metaprogramming),
        Node::Lvar(_) => Err(ParseError::Metaprogramming),
        Node::Ivar(_) => Err(ParseError::Metaprogramming),
        Node::Self_(_) => Err(ParseError::Metaprogramming),
        _node => Err(ParseError::Metaprogramming),
    }
}

fn fetch_const_const_name(node: &nodes::Const) -> Result<String, ParseError> {
    match &node.scope {
        Some(s) => {
            let parent_namespace = fetch_const_name(s)?;
            Ok(format!("{}::{}", parent_namespace, node.name))
        }
        None => Ok(node.name.to_owned()),
    }
}

fn fetch_node_location(node: &nodes::Node) -> Result<&Loc, ParseError> {
    match node {
        Node::Const(const_node) => Ok(&const_node.expression_l),
        node => {
            panic!(
                "Cannot handle other node in get_constant_node_name: {:?}",
                node
            )
        }
    }
}

fn get_definition_from(
    current_nesting: &String,
    parent_nesting: &[String],
    location: &Range,
) -> ParsedDefinition {
    let name = current_nesting.to_owned();

    let owned_namespace_path: Vec<String> = parent_nesting.to_vec();

    let fully_qualified_name = if !owned_namespace_path.is_empty() {
        let mut name_components = owned_namespace_path;
        name_components.push(name);
        format!("::{}", name_components.join("::"))
    } else {
        format!("::{}", name)
    };

    ParsedDefinition {
        fully_qualified_name,
        location: location.to_owned(),
    }
}

fn loc_to_range(loc: &Loc, lookup: &LineColLookup) -> Range {
    let (start_row, start_col) = lookup.get(loc.begin); // There's an off-by-one difference here with packwerk
    let (end_row, end_col) = lookup.get(loc.end);

    Range {
        start_row,
        start_col: start_col - 1,
        end_row,
        end_col,
    }
}

fn get_reference_from_active_record_association(
    node: &nodes::Send,
    current_namespaces: &[String],
    line_col_lookup: &LineColLookup,
    custom_associations: &[String],
) -> Option<UnresolvedReference> {
    // TODO: Read in args, process associations as a separate class
    // These can get complicated! e.g. we can specify a class name
    let combined_associations: Vec<String> = custom_associations
        .iter()
        .map(|s| s.to_owned())
        .chain(ASSOCIATION_METHOD_NAMES.iter().copied().map(String::from))
        .collect();

    let is_association = combined_associations
        .iter()
        .any(|association_method| node.method_name == *association_method);

    if is_association {
        let first_arg: Option<&Node> = node.args.first();

        let mut name: Option<String> = None;
        for node in node.args.iter() {
            if let Node::Kwargs(kwargs) = node {
                if let Some(found) = extract_class_name_from_kwargs(kwargs) {
                    name = Some(found);
                }
            }
        }

        if let Some(Node::Sym(d)) = first_arg {
            if name.is_none() {
                // We singularize here because by convention Rails will singularize the class name as declared via a symbol,
                // e.g. `has_many :companies` will look for a class named `Company`, not `Companies`
                name = Some(to_class_case(
                    &d.name.to_string_lossy(),
                    true,
                    &HashSet::new(), // todo: pass in acronyms here
                ));
            }
        }

        if name.is_some() {
            let unwrapped_name = name.unwrap_or_else(|| {
                panic!("Could not find class name for association {:?}", &node,)
            });

            Some(UnresolvedReference {
                name: unwrapped_name,
                namespace_path: current_namespaces.to_owned(),
                location: loc_to_range(&node.expression_l, line_col_lookup),
            })
        } else {
            None
        }
    } else {
        None
    }
}

fn extract_class_name_from_kwargs(kwargs: &nodes::Kwargs) -> Option<String> {
    for pair_node in kwargs.pairs.iter() {
        if let Node::Pair(pair) = pair_node {
            if let Node::Sym(k) = *pair.key.to_owned() {
                if k.name.to_string_lossy() == *"class_name" {
                    if let Node::Str(v) = *pair.value.to_owned() {
                        return Some(v.value.to_string_lossy());
                    }
                }
            }
        }
    }

    None
}

fn get_constant_assignment_definition(
    node: &nodes::Casgn,
    current_namespaces: Vec<String>,
    line_col_lookup: &LineColLookup,
) -> Option<ParsedDefinition> {
    let name_result = fetch_casgn_name(node);
    if name_result.is_err() {
        return None;
    }

    let name = name_result.unwrap();
    let fully_qualified_name = if !current_namespaces.is_empty() {
        let mut name_components = current_namespaces;
        name_components.push(name);
        format!("::{}", name_components.join("::"))
    } else {
        format!("::{}", name)
    };

    Some(ParsedDefinition {
        fully_qualified_name,
        location: loc_to_range(&node.expression_l, line_col_lookup),
    })
}

fn fetch_casgn_name(node: &nodes::Casgn) -> Result<String, ParseError> {
    match &node.scope {
        Some(s) => {
            let parent_namespace = fetch_const_name(s)?;
            Ok(format!("{}::{}", parent_namespace, node.name))
        }
        None => Ok(node.name.to_owned()),
    }
}
