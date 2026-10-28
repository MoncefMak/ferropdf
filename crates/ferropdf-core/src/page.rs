const MM_TO_PT: f32 = 2.834_646;

#[derive(Debug, Clone, PartialEq)]
pub enum PageSize {
    A3,
    A4,
    A5,
    A6,
    Letter,
    Legal,
    Custom(f32, f32),
}

impl PageSize {
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            PageSize::A3 => (841.89, 1190.55),
            PageSize::A4 => (595.28, 841.89),
            PageSize::A5 => (419.53, 595.28),
            PageSize::A6 => (297.64, 419.53),
            PageSize::Letter => (612.0, 792.0),
            PageSize::Legal => (612.0, 1008.0),
            PageSize::Custom(w, h) => (*w, *h),
        }
    }
    /// Parse a CSS `size` value / `Options.page_size` string.
    ///
    /// Accepts `<page-size> || [portrait | landscape]` in any order
    /// (CSS Paged Media §7.1): `"A4"`, `"A4 landscape"`, `"landscape"`,
    /// `"210mm 297mm"`, `"5in"` (square). Unknown values fall back to A4.
    #[allow(clippy::should_implement_trait)]
    pub fn from_str(s: &str) -> Self {
        let mut orientation = None;
        let rest: Vec<&str> = s
            .split_whitespace()
            .filter(|tok| match tok.to_ascii_lowercase().as_str() {
                "landscape" => {
                    orientation = Some(true);
                    false
                }
                "portrait" => {
                    orientation = Some(false);
                    false
                }
                _ => true,
            })
            .collect();
        let base = Self::from_size_keyword(&rest.join(" "));
        match orientation {
            None => base,
            Some(landscape) => {
                let (w, h) = base.dimensions_pt();
                let (short, long) = if w <= h { (w, h) } else { (h, w) };
                if landscape {
                    PageSize::Custom(long, short)
                } else if w <= h {
                    base
                } else {
                    PageSize::Custom(short, long)
                }
            }
        }
    }

    fn from_size_keyword(s: &str) -> Self {
        match s.trim().to_uppercase().as_str() {
            "" | "AUTO" => PageSize::A4,
            "A3" => PageSize::A3,
            "A4" => PageSize::A4,
            "A5" => PageSize::A5,
            "A6" => PageSize::A6,
            "LETTER" => PageSize::Letter,
            "LEGAL" => PageSize::Legal,
            _ => {
                // Try parsing as custom dimensions: "90mm 76mm", "8.5in 11in", etc.
                if let Some(size) = parse_custom_page_size(s) {
                    size
                } else {
                    PageSize::A4
                }
            }
        }
    }
    pub fn name(&self) -> &'static str {
        match self {
            PageSize::A3 => "A3",
            PageSize::A4 => "A4",
            PageSize::A5 => "A5",
            PageSize::A6 => "A6",
            PageSize::Letter => "Letter",
            PageSize::Legal => "Legal",
            PageSize::Custom(_, _) => "Custom",
        }
    }
}

/// Parse a custom page size string like "90mm 76mm" into a PageSize::Custom.
/// Accepts two whitespace-separated CSS lengths (mm, cm, in, pt, px).
fn parse_custom_page_size(s: &str) -> Option<PageSize> {
    // Split on whitespace to find exactly two tokens with units
    let parts: Vec<&str> = s.split_whitespace().collect();
    if parts.len() == 1 {
        // A single length gives a square page (`size: 10cm`).
        let side = parse_css_length_to_pt(parts[0])?;
        return (side > 0.0).then_some(PageSize::Custom(side, side));
    }
    if parts.len() == 2 {
        let w = parse_css_length_to_pt(parts[0])?;
        let h = parse_css_length_to_pt(parts[1])?;
        if w > 0.0 && h > 0.0 {
            return Some(PageSize::Custom(w, h));
        }
    }
    None
}

#[derive(Debug, Clone)]
pub enum Orientation {
    Portrait,
    Landscape,
}

#[derive(Debug, Clone)]
pub struct PageMargins {
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
    pub left: f32,
}

impl Default for PageMargins {
    fn default() -> Self {
        Self::mm(20.0, 20.0, 20.0, 20.0)
    }
}

impl PageMargins {
    pub fn mm(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        Self {
            top: top * MM_TO_PT,
            right: right * MM_TO_PT,
            bottom: bottom * MM_TO_PT,
            left: left * MM_TO_PT,
        }
    }
    pub fn uniform_mm(v: f32) -> Self {
        Self::mm(v, v, v, v)
    }
    pub fn uniform_pt(v: f32) -> Self {
        Self {
            top: v,
            right: v,
            bottom: v,
            left: v,
        }
    }
    pub fn from_css_str(s: &str) -> Self {
        let v = parse_css_length_to_pt(s).unwrap_or(56.69); // 20mm par défaut
        Self::uniform_pt(v)
    }
}

fn parse_css_length_to_pt(s: &str) -> Option<f32> {
    let s = s.trim();
    if let Some(v) = s.strip_suffix("mm") {
        v.trim().parse::<f32>().ok().map(|v| v * MM_TO_PT)
    } else if let Some(v) = s.strip_suffix("cm") {
        v.trim().parse::<f32>().ok().map(|v| v * MM_TO_PT * 10.0)
    } else if let Some(v) = s.strip_suffix("in") {
        v.trim().parse::<f32>().ok().map(|v| v * 72.0)
    } else if let Some(v) = s.strip_suffix("pt") {
        v.trim().parse::<f32>().ok()
    } else if let Some(v) = s.strip_suffix("px") {
        v.trim().parse::<f32>().ok().map(|v| v * 0.75)
    } else {
        None
    }
}

#[derive(Debug, Clone)]
pub struct PageConfig {
    pub size: PageSize,
    pub margins: PageMargins,
    pub orientation: Orientation,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            size: PageSize::A4,
            margins: PageMargins::default(),
            orientation: Orientation::Portrait,
        }
    }
}

impl PageConfig {
    /// Content width in points (pt). Unité interne du moteur.
    pub fn content_width_pt(&self) -> f32 {
        let (w, _) = self.size.dimensions_pt();
        (w - self.margins.left - self.margins.right).max(0.0)
    }
    /// Content height in points (pt). Unité interne du moteur.
    pub fn content_height_pt(&self) -> f32 {
        let (_, h) = self.size.dimensions_pt();
        (h - self.margins.top - self.margins.bottom).max(0.0)
    }
    /// Page width in points (pt).
    pub fn page_width_pt(&self) -> f32 {
        self.size.dimensions_pt().0
    }
    /// Page height in points (pt).
    pub fn page_height_pt(&self) -> f32 {
        self.size.dimensions_pt().1
    }
}

#[cfg(test)]
mod orientation_tests {
    use super::*;

    fn dims(s: &str) -> (i32, i32) {
        let (w, h) = PageSize::from_str(s).dimensions_pt();
        (w.round() as i32, h.round() as i32)
    }

    #[test]
    fn named_sizes_with_orientation() {
        assert_eq!(dims("A4"), (595, 842));
        assert_eq!(dims("A4 landscape"), (842, 595));
        assert_eq!(dims("landscape A4"), (842, 595));
        assert_eq!(dims("a5 Landscape"), (595, 420));
        assert_eq!(dims("letter portrait"), (612, 792));
    }

    #[test]
    fn orientation_only_applies_to_a4() {
        assert_eq!(dims("landscape"), (842, 595));
        assert_eq!(dims("portrait"), (595, 842));
    }

    #[test]
    fn explicit_lengths() {
        assert_eq!(dims("297mm 210mm"), (842, 595));
        assert_eq!(dims("210mm 297mm landscape"), (842, 595));
        assert_eq!(dims("297mm 210mm portrait"), (595, 842));
        assert_eq!(dims("10cm"), (283, 283));
        assert_eq!(dims("auto"), (595, 842));
    }
}
