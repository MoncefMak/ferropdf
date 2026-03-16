//! Font management and caching.

pub mod metrics;
pub mod shaping;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use parking_lot::RwLock;

use crate::error::{FastPdfError, Result};

/// A loaded font resource.
#[derive(Debug, Clone)]
pub struct FontData {
    pub family: String,
    pub weight: u32,
    pub italic: bool,
    pub data: Arc<Vec<u8>>,
    pub path: Option<PathBuf>,
}

/// Thread-safe font cache.
pub struct FontCache {
    fonts: RwLock<HashMap<FontKey, FontData>>,
    search_paths: RwLock<Vec<PathBuf>>,
}

#[derive(Debug, Clone, Hash, PartialEq, Eq)]
pub struct FontKey {
    pub family: String,
    pub weight: u32,
    pub italic: bool,
}

impl FontCache {
    pub fn new() -> Self {
        Self {
            fonts: RwLock::new(HashMap::new()),
            search_paths: RwLock::new(Vec::new()),
        }
    }

    /// Add a directory to search for font files.
    pub fn add_search_path(&self, path: impl Into<PathBuf>) {
        self.search_paths.write().push(path.into());
    }

    /// Register a font from raw bytes.
    pub fn register_font(
        &self,
        family: &str,
        weight: u32,
        italic: bool,
        data: Vec<u8>,
    ) -> Result<()> {
        let key = FontKey {
            family: family.to_lowercase(),
            weight,
            italic,
        };

        self.fonts.write().insert(
            key,
            FontData {
                family: family.to_string(),
                weight,
                italic,
                data: Arc::new(data),
                path: None,
            },
        );

        Ok(())
    }

    /// Register a font from a file path.
    pub fn register_font_file(
        &self,
        family: &str,
        weight: u32,
        italic: bool,
        path: impl AsRef<Path>,
    ) -> Result<()> {
        let data = std::fs::read(path.as_ref()).map_err(|e| {
            FastPdfError::Font(format!(
                "Failed to read font file {:?}: {}",
                path.as_ref(),
                e
            ))
        })?;

        let key = FontKey {
            family: family.to_lowercase(),
            weight,
            italic,
        };

        self.fonts.write().insert(
            key,
            FontData {
                family: family.to_string(),
                weight,
                italic,
                data: Arc::new(data),
                path: Some(path.as_ref().to_path_buf()),
            },
        );

        Ok(())
    }

    /// Look up a font by family, weight, and style.
    pub fn get_font(&self, family: &str, weight: u32, italic: bool) -> Option<FontData> {
        let key = FontKey {
            family: family.to_lowercase(),
            weight,
            italic,
        };

        let fonts = self.fonts.read();

        // Exact match
        if let Some(font) = fonts.get(&key) {
            return Some(font.clone());
        }

        // Try without italic
        if italic {
            let fallback_key = FontKey {
                family: family.to_lowercase(),
                weight,
                italic: false,
            };
            if let Some(font) = fonts.get(&fallback_key) {
                return Some(font.clone());
            }
        }

        // Try with normal weight (400)
        let normal_key = FontKey {
            family: family.to_lowercase(),
            weight: 400,
            italic: false,
        };
        if let Some(font) = fonts.get(&normal_key) {
            return Some(font.clone());
        }

        // Try any font in the family
        fonts
            .values()
            .find(|f| f.family.to_lowercase() == family.to_lowercase())
            .cloned()
    }

    /// Get the number of cached fonts.
    pub fn font_count(&self) -> usize {
        self.fonts.read().len()
    }

    /// Register standard built-in fonts.
    pub fn register_standard_fonts(&self) {
        // Standard PDF fonts don't need external data
        // They are built into PDF readers
        let standard_fonts = [
            ("serif", 400, false),
            ("serif", 700, false),
            ("serif", 400, true),
            ("serif", 700, true),
            ("sans-serif", 400, false),
            ("sans-serif", 700, false),
            ("sans-serif", 400, true),
            ("sans-serif", 700, true),
            ("monospace", 400, false),
            ("monospace", 700, false),
        ];

        for (family, weight, italic) in &standard_fonts {
            let key = FontKey {
                family: family.to_string(),
                weight: *weight,
                italic: *italic,
            };

            self.fonts.write().insert(
                key,
                FontData {
                    family: family.to_string(),
                    weight: *weight,
                    italic: *italic,
                    data: Arc::new(Vec::new()), // Empty — use PDF built-in
                    path: None,
                },
            );
        }
    }
}

impl Default for FontCache {
    fn default() -> Self {
        let cache = Self::new();
        cache.register_standard_fonts();
        cache
    }
}

/// Map CSS font family to PDF built-in font name.
pub fn css_to_pdf_font(family: &str, weight: u32, italic: bool) -> &'static str {
    let family_lower = family.to_lowercase();
    let is_bold = weight >= 700;

    match family_lower.as_str() {
        "serif" | "times" | "times new roman" | "georgia" => match (is_bold, italic) {
            (true, true) => "Times-BoldItalic",
            (true, false) => "Times-Bold",
            (false, true) => "Times-Italic",
            (false, false) => "Times-Roman",
        },
        "sans-serif" | "arial" | "helvetica" | "verdana" | "tahoma" => match (is_bold, italic) {
            (true, true) => "Helvetica-BoldOblique",
            (true, false) => "Helvetica-Bold",
            (false, true) => "Helvetica-Oblique",
            (false, false) => "Helvetica",
        },
        "monospace" | "courier" | "courier new" | "consolas" => match (is_bold, italic) {
            (true, true) => "Courier-BoldOblique",
            (true, false) => "Courier-Bold",
            (false, true) => "Courier-Oblique",
            (false, false) => "Courier",
        },
        _ => {
            // Default to Helvetica for unknown fonts
            match (is_bold, italic) {
                (true, true) => "Helvetica-BoldOblique",
                (true, false) => "Helvetica-Bold",
                (false, true) => "Helvetica-Oblique",
                (false, false) => "Helvetica",
            }
        }
    }
}
