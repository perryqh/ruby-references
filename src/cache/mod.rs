use std::{
    path::{Path, PathBuf},
    sync::Arc,
};

use anyhow::Context;
use tokio::io::AsyncReadExt;

use crate::parser::ProcessedFile;

use self::{cache::Cache, cached_file::CachedFile, cached_noop::NoopCache};

pub(crate) mod cache;
pub(crate) mod cached_file;
pub(crate) mod cached_noop;

pub(crate) fn get_cache(enabled: bool, cache_dir: PathBuf) -> Arc<dyn Cache + Send + Sync> {
    if enabled {
        Arc::new(CachedFile { cache_dir })
    } else {
        Arc::new(NoopCache {})
    }
}

pub enum CacheResult {
    Processed(ProcessedFile),
    Miss(EmptyCacheEntry),
}

#[derive(Debug, Default)]
pub struct EmptyCacheEntry {
    pub filepath: PathBuf,
    pub file_contents_digest: String,
    pub file_name_digest: String,
    pub cache_file_path: PathBuf,
}

impl EmptyCacheEntry {
    pub async fn new(cache_directory: &Path, filepath: &Path) -> anyhow::Result<EmptyCacheEntry> {
        let file_digest = md5::compute(filepath.to_str().unwrap());
        let file_name_digest = format!("{:x}", file_digest);
        let cache_file_path = cache_file_path_from_digest(cache_directory, &file_name_digest);

        let file_contents_digest = file_content_digest(filepath).await?;

        Ok(EmptyCacheEntry {
            filepath: filepath.to_owned(),
            file_contents_digest,
            cache_file_path,
            file_name_digest,
        })
    }
}

pub async fn create_cache_dir_idempotently(cache_dir: &Path) -> anyhow::Result<()> {
    tokio::fs::create_dir_all(cache_dir)
        .await
        .context("Failed to create cache directory")
}

pub(crate) async fn file_content_digest(file: &Path) -> anyhow::Result<String> {
    let mut file_content = Vec::new();

    // Read the file content
    let mut file_handle = tokio::fs::File::open(file)
        .await
        .context(format!("Failed to open file {:?}", file))?;
    file_handle
        .read_to_end(&mut file_content)
        .await
        .context(format!("Failed to read file {:?}", file))?;

    // Compute the MD5 digest
    Ok(format!("{:x}", md5::compute(&file_content)))
}

// This function is used to generate the cache file path from the digest of the file name
// The cache file path is a directory structure with the first two characters of the digest as the directory name
// and the rest of the digest as the file name
fn cache_file_path_from_digest(cache_directory: &Path, file_name_digest: &str) -> PathBuf {
    let cached_directory_for_digest = cache_directory.join(&file_name_digest[..2]);
    cached_directory_for_digest.join(&file_name_digest[2..])
}
