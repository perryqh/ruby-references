use crate::parser::ProcessedFile;
use std::path::Path;

use super::{CacheResult, EmptyCacheEntry};

#[async_trait::async_trait]
pub trait Cache {
    async fn get(&self, path: &Path) -> anyhow::Result<CacheResult>;

    async fn write(
        &self,
        empty_cache_entry: &EmptyCacheEntry,
        processed_file: &ProcessedFile,
    ) -> anyhow::Result<()>;
}
