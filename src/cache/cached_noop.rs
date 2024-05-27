use std::path::Path;

use crate::parser::ProcessedFile;

use super::{cache::Cache, CacheResult, EmptyCacheEntry};

pub struct NoopCache {}

#[async_trait::async_trait]
impl Cache for NoopCache {
    async fn get(&self, _path: &Path) -> anyhow::Result<CacheResult> {
        // Return nothing!
        Ok(CacheResult::Miss(EmptyCacheEntry::default()))
    }

    async fn write(
        &self,
        _empty_cache_entry: &EmptyCacheEntry,
        _processed_file: &ProcessedFile,
    ) -> anyhow::Result<()> {
        // Do nothing!
        Ok(())
    }
}
