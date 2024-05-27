use serde::{Deserialize, Serialize};

use anyhow::Context;
use std::path::Path;
use std::path::PathBuf;
use tokio::io::AsyncReadExt;
use tokio::io::AsyncWriteExt;
use tracing::warn;

use crate::parser::ProcessedFile;

use super::cache::Cache;
use super::create_cache_dir_idempotently;
use super::CacheResult;
use super::EmptyCacheEntry;

pub struct CachedFile {
    pub cache_dir: PathBuf,
}

#[async_trait::async_trait]
impl Cache for CachedFile {
    async fn get(&self, path: &Path) -> anyhow::Result<CacheResult> {
        let empty_cache_entry = EmptyCacheEntry::new(&self.cache_dir, path)
            .await
            .context(format!("Failed to create cache entry for {:?}", path))?;
        let cache_entry = CacheEntry::from_empty(&empty_cache_entry).await?;
        if let Some(cache_entry) = cache_entry {
            let file_digests_match =
                cache_entry.file_contents_digest == empty_cache_entry.file_contents_digest;

            if !file_digests_match {
                Ok(CacheResult::Miss(empty_cache_entry))
            } else {
                let processed_file = cache_entry.processed_file;
                Ok(CacheResult::Processed(processed_file))
            }
        } else {
            Ok(CacheResult::Miss(empty_cache_entry))
        }
    }

    async fn write(
        &self,
        empty_cache_entry: &EmptyCacheEntry,
        processed_file: &ProcessedFile,
    ) -> anyhow::Result<()> {
        let file_contents_digest = empty_cache_entry.file_contents_digest.to_owned();

        let cache_entry = &CacheEntry {
            file_contents_digest,
            // Ideally we could pass by reference here, but in practice this cost should be paid on few files
            // that have changed and need to be reprocessed.
            processed_file: processed_file.clone(),
        };

        let cache_data =
            serde_json::to_string(&cache_entry).context("Failed to serialize references")?;
        let mut file = match tokio::fs::File::create(&empty_cache_entry.cache_file_path).await {
            Ok(file) => file,
            Err(_e) => {
                let parent_dir = empty_cache_entry.cache_file_path.parent().context(format!(
                    "Failed to get parent directory for {:?}",
                    empty_cache_entry.cache_file_path
                ))?;
                create_cache_dir_idempotently(parent_dir).await?;
                tokio::fs::File::create(&empty_cache_entry.cache_file_path)
                    .await
                    .context("failed to create cache file")?
            }
        };

        file.write_all(cache_data.as_bytes())
            .await
            .context("Failed to write cache file")?;
        Ok(())
    }
}

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq)]
pub struct CacheEntry {
    pub file_contents_digest: String,
    pub processed_file: ProcessedFile,
}

impl CacheEntry {
    // todo async
    pub async fn from_empty(empty: &EmptyCacheEntry) -> anyhow::Result<Option<CacheEntry>> {
        let cache_file_path = &empty.cache_file_path;

        if cache_file_path.exists() {
            match read_json_file(cache_file_path).await {
                Ok(cache_entry) => Ok(Some(cache_entry)),
                Err(e) => {
                    warn!("Failed to read cache file {:?}: {}", cache_file_path, e);
                    Ok(None)
                }
            }
        } else {
            Ok(None)
        }
    }
}

pub async fn read_json_file(path: &PathBuf) -> anyhow::Result<CacheEntry> {
    let file = tokio::fs::File::open(path)
        .await
        .context(format!("Failed to open file {:?}", path))?;
    let mut reader = tokio::io::BufReader::new(file);
    let mut contents = Vec::new();
    reader
        .read_to_end(&mut contents)
        .await
        .context("Failed to read file contents")?;
    let data = serde_json::from_slice(&contents).context("Failed to deserialize CacheEntry")?;
    Ok(data)
}

#[cfg(test)]
mod tests {
    use std::fs;

    use crate::{
        cache::file_content_digest,
        parser::{Range, UnresolvedReference},
    };

    use super::*;

    fn teardown() {}

    #[tokio::test]
    async fn test_file_content_digest() -> anyhow::Result<()> {
        let file_path = "tests/fixtures/simple_app/packs/bar/app/services/bar.rb";
        let expected_digest = "305bc58696c2e664057b6751064cf2e3";

        let digest = file_content_digest(&PathBuf::from(file_path)).await;

        assert!(digest.is_ok());
        assert_eq!(digest.unwrap(), expected_digest);

        teardown();
        Ok(())
    }

    #[tokio::test]
    async fn test_compatible_with_packwerk() -> anyhow::Result<()> {
        let contents: String = String::from(
            r#"{
  "file_contents_digest":"8f9efdcf2caa22fb7b1b4a8274e68d11",
  "processed_file": {
    "absolute_path":"/tests/fixtures/simple_app/packs/foo/app/services/bar/foo.rb",
    "unresolved_references":[
      {
        "name":"Bar",
        "namespace_path":["Foo","Bar"],
        "location":{"start_row":8,"start_col":22,"end_row":8,"end_col":25}
      }],
    "definitions":[]
  }
}"#,
        );

        let expected_serialized = CacheEntry {
            file_contents_digest: "8f9efdcf2caa22fb7b1b4a8274e68d11".to_owned(),
            processed_file: ProcessedFile {
                absolute_path: PathBuf::from(
                    "/tests/fixtures/simple_app/packs/foo/app/services/bar/foo.rb",
                ),
                unresolved_references: vec![UnresolvedReference {
                    name: "Bar".to_owned(),
                    namespace_path: vec!["Foo".to_owned(), "Bar".to_owned()],
                    location: Range {
                        start_row: 8,
                        start_col: 22,
                        end_row: 8,
                        end_col: 25,
                    },
                }],
            },
        };

        let actual_serialized = serde_json::from_str::<CacheEntry>(&contents).unwrap();

        assert_eq!(expected_serialized, actual_serialized);

        teardown();
        Ok(())
    }

    #[tokio::test]
    async fn test_corrupt_cache() -> anyhow::Result<()> {
        let sha = "e57a05216069923190a4e03d264d9677";
        let corrupt_contents: String = String::from(
            r#"{
  "file_contents_digest":"e57a05216069923190a4e03d264d9677",
  "processed_file": 
}"#,
        );

        let cache_path = PathBuf::from("tests/fixtures/simple_app/tmp/cache/");
        fs::create_dir_all(&cache_path).context("unable to create cache dir")?;
        let corrupt_file_path = cache_path.join(format!("{}", sha));
        fs::write(&corrupt_file_path, corrupt_contents)
            .context("expected to write corrupt cache file")?;

        let empty_cache_entry = EmptyCacheEntry::new(
            &cache_path,
            &PathBuf::from("tests/fixtures/simple_app/packs/foo/app/services/foo/bar.rb"),
        )
        .await
        .context("expected tests/fixtures/simple_app/packs/foo/app/services/foo/bar.rb to exist")?;

        let entry = CacheEntry::from_empty(&empty_cache_entry).await?;
        assert!(entry.is_none());

        Ok(())
    }
}
