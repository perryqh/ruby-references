use std::collections::HashMap;

use super::{
    collector::ReferenceCollector, namespace_calculator::possible_fully_qualified_constants,
    ParsedDefinition, Range, UnresolvedReference,
};

pub fn filter(reference_collector: ReferenceCollector) -> Vec<UnresolvedReference> {
    let definition_to_location_map = definition_to_location_map(&reference_collector.definitions);
    reference_collector
        .references
        .into_iter()
        .filter(|r| {
            let mut should_ignore_local_reference = false;
            let namespace_path = r
                .namespace_path
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<&str>>();
            let possible_constants = possible_fully_qualified_constants(&namespace_path, &r.name);
            for constant_name in possible_constants {
                if let Some(location) = definition_to_location_map
                    .get(&constant_name)
                    .or(definition_to_location_map.get(&format!("::{}", constant_name)))
                {
                    let reference_is_definition = location.start_row == r.location.start_row
                        && location.start_col == r.location.start_col;
                    // In lib/packwerk/parsed_constant_definitions.rb, we don't count references when the reference is in the same place as the definition
                    // This is an idiosyncracy we are porting over here for behavioral alignment, although we might be doing some unnecessary work.
                    should_ignore_local_reference = !reference_is_definition;
                }
            }
            !should_ignore_local_reference
        })
        .collect()
}

fn definition_to_location_map(definitions: &Vec<ParsedDefinition>) -> HashMap<String, Range> {
    let mut definition_to_location_map: HashMap<String, Range> = HashMap::new();
    for d in definitions {
        let parts: Vec<&str> = d.fully_qualified_name.split("::").collect();
        // We do this to handle nested constants, e.g.
        // class Foo::Bar
        // end
        for (index, _) in parts.iter().enumerate() {
            let combined = &parts[..=index].join("::");
            // If the map already contains the key, skip it.
            // This is helpful, e.g.
            // class Foo::Bar
            //  BAZ
            // end
            // The fully name for BAZ IS ::Foo::Bar::BAZ, so we do not want to overwrite
            // the definition location for ::Foo or ::Foo::Bar
            if !definition_to_location_map.contains_key(combined) {
                definition_to_location_map.insert(combined.to_owned(), d.location.clone());
            }
        }
    }
    definition_to_location_map
}
