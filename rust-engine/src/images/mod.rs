//! Image handling and caching.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::error::{FastPdfError, Result};

/// Supported image formats.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Gif,
    Svg,
    Bmp,
    Webp,
}

impl ImageFormat {
    pub fn from_extension(ext: &str) -> Option<Self> {
        match ext.to_lowercase().as_str() {
            "png" => Some(Self::Png),
            "jpg" | "jpeg" => Some(Self::Jpeg),
            "gif" => Some(Self::Gif),
            "svg" => Some(Self::Svg),
            "bmp" => Some(Self::Bmp),
            "webp" => Some(Self::Webp),
            _ => None,
        }
    }

    pub fn from_mime(mime: &str) -> Option<Self> {
        match mime {
            "image/png" => Some(Self::Png),
            "image/jpeg" => Some(Self::Jpeg),
            "image/gif" => Some(Self::Gif),
            "image/svg+xml" => Some(Self::Svg),
            "image/bmp" => Some(Self::Bmp),
            "image/webp" => Some(Self::Webp),
            _ => None,
        }
    }
}

/// A loaded image resource.
#[derive(Debug, Clone)]
pub struct ImageData {
    pub format: ImageFormat,
    pub data: Arc<Vec<u8>>,
    pub width: u32,
    pub height: u32,
}

/// Thread-safe image cache.
pub struct ImageCache {
    images: RwLock<HashMap<String, ImageData>>,
    base_path: RwLock<Option<PathBuf>>,
}

impl ImageCache {
    pub fn new() -> Self {
        Self {
            images: RwLock::new(HashMap::new()),
            base_path: RwLock::new(None),
        }
    }

    /// Set the base path for resolving relative image paths.
    pub fn set_base_path(&self, path: impl Into<PathBuf>) {
        *self.base_path.write() = Some(path.into());
    }

    /// Load an image from a source (file path, URL, or data URI).
    pub fn load_image(&self, src: &str) -> Result<ImageData> {
        // Check cache
        if let Some(cached) = self.images.read().get(src) {
            return Ok(cached.clone());
        }

        let image_data = if src.starts_with("data:") {
            self.load_data_uri(src)?
        } else if src.starts_with("http://") || src.starts_with("https://") {
            // For now, skip remote images
            return Err(FastPdfError::Image(
                "Remote images not yet supported; use local paths or data URIs".to_string(),
            ));
        } else {
            self.load_file(src)?
        };

        // Cache it
        self.images
            .write()
            .insert(src.to_string(), image_data.clone());

        Ok(image_data)
    }

    /// Load from a data: URI (e.g., data:image/png;base64,...).
    fn load_data_uri(&self, uri: &str) -> Result<ImageData> {
        let uri = uri.strip_prefix("data:").unwrap_or(uri);
        let parts: Vec<&str> = uri.splitn(2, ',').collect();
        if parts.len() != 2 {
            return Err(FastPdfError::Image("Invalid data URI".to_string()));
        }

        let meta = parts[0];
        let data_str = parts[1];

        let format = if meta.contains("image/png") {
            ImageFormat::Png
        } else if meta.contains("image/jpeg") {
            ImageFormat::Jpeg
        } else if meta.contains("image/svg+xml") {
            ImageFormat::Svg
        } else if meta.contains("image/gif") {
            ImageFormat::Gif
        } else {
            ImageFormat::Png // Default
        };

        let data = if meta.contains("base64") {
            use base64::Engine;
            base64::engine::general_purpose::STANDARD
                .decode(data_str)
                .map_err(|e| FastPdfError::Image(format!("Base64 decode error: {}", e)))?
        } else {
            data_str.as_bytes().to_vec()
        };

        let (width, height) = Self::get_image_dimensions(&data, format)?;

        Ok(ImageData {
            format,
            data: Arc::new(data),
            width,
            height,
        })
    }

    /// Load from a file path.
    fn load_file(&self, path: &str) -> Result<ImageData> {
        let file_path = if Path::new(path).is_absolute() {
            PathBuf::from(path)
        } else if let Some(base) = self.base_path.read().as_ref() {
            base.join(path)
        } else {
            PathBuf::from(path)
        };

        let data = std::fs::read(&file_path).map_err(|e| {
            FastPdfError::Image(format!("Failed to read image {:?}: {}", file_path, e))
        })?;

        let ext = file_path
            .extension()
            .and_then(|e| e.to_str())
            .unwrap_or("png");

        let format = ImageFormat::from_extension(ext)
            .ok_or_else(|| FastPdfError::Image(format!("Unsupported image format: {}", ext)))?;

        let (width, height) = Self::get_image_dimensions(&data, format)?;

        Ok(ImageData {
            format,
            data: Arc::new(data),
            width,
            height,
        })
    }

    /// Get image dimensions from raw data.
    fn get_image_dimensions(data: &[u8], format: ImageFormat) -> Result<(u32, u32)> {
        match format {
            ImageFormat::Svg => {
                // SVG dimensions are parsed later by the renderer
                Ok((300, 150))
            }
            _ => {
                // Use the image crate for raster formats
                let reader = image::io::Reader::new(std::io::Cursor::new(data))
                    .with_guessed_format()
                    .map_err(|e| {
                        FastPdfError::Image(format!("Failed to detect image format: {}", e))
                    })?;

                let dims = reader.into_dimensions().map_err(|e| {
                    FastPdfError::Image(format!("Failed to read image dimensions: {}", e))
                })?;

                Ok(dims)
            }
        }
    }

    /// Clear the image cache.
    pub fn clear(&self) {
        self.images.write().clear();
    }

    pub fn cached_count(&self) -> usize {
        self.images.read().len()
    }
}

impl Default for ImageCache {
    fn default() -> Self {
        Self::new()
    }
}
