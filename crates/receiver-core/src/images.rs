use std::path::{Path, PathBuf};

use tokio::fs;
use tokio::io::AsyncWriteExt;

use crate::Result;

pub const IMAGE_BASE_URL: &str = "https://receiver.808bits.com/images/";

#[derive(Debug, Clone)]
pub struct ImageCache {
    cache_dir: PathBuf,
    base_url: String,
    client: reqwest::Client,
}

impl ImageCache {
    pub fn new(cache_dir: impl AsRef<Path>) -> Self {
        Self::with_base_url(cache_dir, IMAGE_BASE_URL)
    }

    pub fn with_base_url(cache_dir: impl AsRef<Path>, base_url: impl Into<String>) -> Self {
        Self {
            cache_dir: cache_dir.as_ref().to_path_buf(),
            base_url: base_url.into(),
            client: reqwest::Client::new(),
        }
    }

    pub fn cache_path(&self, image_hash: i64) -> PathBuf {
        self.cache_dir.join(image_hash.to_string())
    }

    pub async fn load_bytes(&self, image_hash: i64) -> Result<Option<Vec<u8>>> {
        if image_hash == 0 {
            return Ok(None);
        }
        let path = self.cache_path(image_hash);
        if path.exists() {
            return Ok(Some(fs::read(path).await?));
        }

        fs::create_dir_all(&self.cache_dir).await?;
        let url = format!("{}{}", self.base_url, image_hash);
        let response = self.client.get(url).send().await?.error_for_status()?;
        let bytes = response.bytes().await?;
        let mut file = fs::File::create(path).await?;
        file.write_all(&bytes).await?;
        Ok(Some(bytes.to_vec()))
    }
}
