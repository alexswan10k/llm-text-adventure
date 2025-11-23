use anyhow::Result;
use crate::model::Location;
use std::path::PathBuf;

#[async_trait::async_trait]
pub trait ImageCache {
    async fn get_cached_path(&self, location_id: &str) -> Option<String>;
    async fn save_image(&self, location_id: &str, data: &[u8]) -> Result<String>;
}

#[async_trait::async_trait]
pub trait ImageGenerator {
    async fn generate_image(&self, prompt: &str) -> Result<Vec<u8>>;
}

pub struct ImageManager<C: ImageCache, G: ImageGenerator> {
    cache: C,
    generator: G,
}

impl<C: ImageCache, G: ImageGenerator> ImageManager<C, G> {
    pub fn new(cache: C, generator: G) -> Self {
        Self { cache, generator }
    }

    pub async fn get_image_for_location(&self, location: &Location) -> Result<String> {
        if let Some(path) = self.cache.get_cached_path(&location.id).await {
            return Ok(path);
        }

        // Generate
        let data = self.generator.generate_image(&location.image_prompt).await?;
        let path = self.cache.save_image(&location.id, &data).await?;
        
        Ok(path)
    }
}

// --- Implementations ---

pub struct MockImageGenerator;

#[async_trait::async_trait]
impl ImageGenerator for MockImageGenerator {
    async fn generate_image(&self, _prompt: &str) -> Result<Vec<u8>> {
        // Return a dummy 1x1 pixel or similar
        Ok(vec![0; 10])
    }
}

#[cfg(not(target_arch = "wasm32"))]
pub struct FileSystemCache {
    base_dir: PathBuf,
}

#[cfg(not(target_arch = "wasm32"))]
impl FileSystemCache {
    pub fn new(base_dir: PathBuf) -> Self {
        std::fs::create_dir_all(&base_dir).unwrap_or_default();
        Self { base_dir }
    }
}

#[cfg(not(target_arch = "wasm32"))]
#[async_trait::async_trait]
impl ImageCache for FileSystemCache {
    async fn get_cached_path(&self, location_id: &str) -> Option<String> {
        let path = self.base_dir.join(format!("{}.png", location_id));
        if path.exists() {
            Some(path.to_string_lossy().to_string())
        } else {
            None
        }
    }

    async fn save_image(&self, location_id: &str, data: &[u8]) -> Result<String> {
        let path = self.base_dir.join(format!("{}.png", location_id));
        tokio::fs::write(&path, data).await?;
        Ok(path.to_string_lossy().to_string())
    }
}

#[cfg(target_arch = "wasm32")]
pub struct InMemoryCache {
    // In a real app, use a HashMap<String, String> (Url)
}

#[cfg(target_arch = "wasm32")]
impl InMemoryCache {
    pub fn new() -> Self { Self {} }
}

#[cfg(target_arch = "wasm32")]
#[async_trait::async_trait]
impl ImageCache for InMemoryCache {
    async fn get_cached_path(&self, _location_id: &str) -> Option<String> {
        None
    }

    async fn save_image(&self, _location_id: &str, _data: &[u8]) -> Result<String> {
        // Create Blob URL
        Ok("blob:dummy".to_string())
    }
}
