use std::{
    collections::HashMap,
    path::{Path, PathBuf},
};

use serde::{Deserialize, Serialize};

use crate::references::{
    cache::create_cache_dir_idempotently, constant_resolver::ConstantDefinition,
};

#[derive(Serialize, Deserialize)]
pub(crate) struct ConstantResolverCache {
    pub(crate) file_definition_map: HashMap<PathBuf, String>,
}

pub(crate) fn get_constant_resolver_cache(cache_dir: &Path) -> ConstantResolverCache {
    let path = cache_dir.join("constant_resolver.json");
    if path.exists() {
        let file = std::fs::File::open(path).unwrap();
        let reader = std::io::BufReader::new(file);
        serde_json::from_reader(reader).unwrap()
    } else {
        ConstantResolverCache {
            file_definition_map: HashMap::new(),
        }
    }
}

pub(crate) fn write_cache_constant_definitions(
    constants: &Vec<ConstantDefinition>,
    cache_dir: &Path,
    cache_enabled: bool,
) {
    if !cache_enabled {
        return;
    }

    let mut file_definition_map: HashMap<PathBuf, String> = HashMap::new();
    for constant in constants {
        file_definition_map.insert(
            constant.absolute_path_of_definition.clone(),
            constant.fully_qualified_name.clone(),
        );
    }

    let cache_data_json = serde_json::to_string(&ConstantResolverCache {
        file_definition_map,
    })
    .expect("Failed to serialize");

    let _ = create_cache_dir_idempotently(cache_dir);
    std::fs::write(cache_dir.join("constant_resolver.json"), cache_data_json).unwrap();
}
