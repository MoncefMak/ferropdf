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
    /// Parse the `margin` shorthand: 1 to 4 lengths (top right bottom left,
    /// CSS order). Invalid input falls back to 20mm on every side.
    pub fn from_css_str(s: &str) -> Self {
        let vals: Option<Vec<f32>> = s.split_whitespace().map(parse_css_length_to_pt).collect();
        let [top, right, bottom, left] = match vals.as_deref() {
            Some([a]) => [*a, *a, *a, *a],
            Some([v, h]) => [*v, *h, *v, *h],
            Some([t, h, b]) => [*t, *h, *b, *h],
            Some([t, r, b, l]) => [*t, *r, *b, *l],
            _ => [56.69; 4], // 20mm par défaut
        };
        Self {
            top,
            right,
            bottom,
            left,
        }
    }

    /// Parse the `margin` shorthand strictly: `None` when invalid.
    pub fn parse_shorthand(s: &str) -> Option<[f32; 4]> {
        let vals: Option<Vec<f32>> = s.split_whitespace().map(parse_css_length_to_pt).collect();
        match vals.as_deref()? {
            [a] => Some([*a; 4]),
            [v, h] => Some([*v, *h, *v, *h]),
            [t, h, b] => Some([*t, *h, *b, *h]),
            [t, r, b, l] => Some([*t, *r, *b, *l]),
            _ => None,
        }
    }

    /// Parse one margin length; `None` when invalid.
    pub fn parse_side(s: &str) -> Option<f32> {
        parse_css_length_to_pt(s)
    }

    /// Apply a single side (`margin-top: 3cm`); invalid values are ignored.
    pub fn set_side(&mut self, side: usize, value: &str) {
        if let Some(v) = parse_css_length_to_pt(value) {
            match side {
                0 => self.top = v,
                1 => self.right = v,
                2 => self.bottom = v,
                _ => self.left = v,
            }
        }
    }
}

fn parse_css_length_to_pt(s: &str) -> Option<f32> {
    let s = s.trim();
    if s == "0" {
        return Some(0.0);
    }
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

/// Margin sides (top, right, bottom, left) set by `@page :first`,
/// `@page :left` and `@page :right` rules; `None` keeps the base margin.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct PageMarginOverrides {
    pub first: [Option<f32>; 4],
    pub left: [Option<f32>; 4],
    pub right: [Option<f32>; 4],
}

#[derive(Debug, Clone)]
pub struct PageConfig {
    pub size: PageSize,
    /// Margins of the base `@page` rule (and the layout width).
    pub margins: PageMargins,
    pub orientation: Orientation,
    /// Per-page margins from `:first` / `:left` / `:right`.
    pub margin_overrides: PageMarginOverrides,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            size: PageSize::A4,
            margins: PageMargins::default(),
            orientation: Orientation::Portrait,
            margin_overrides: PageMarginOverrides::default(),
        }
    }
}

impl PageConfig {
    /// Margins of page `page_number` (1-based). Page 1 is a right page;
    /// `:first` beats `:left` / `:right`, which beat the base rule
    /// (CSS Paged Media 3 §4.2).
    ///
    /// Content is laid out once at the base content width: a per-page
    /// left margin moves the content horizontally (binding offset) but does
    /// not re-flow it.
    pub fn margins_for(&self, page_number: u32) -> PageMargins {
        let mut m = self.margins.clone();
        let mut apply = |sides: &[Option<f32>; 4]| {
            for (i, v) in sides.iter().enumerate() {
                if let Some(v) = v {
                    match i {
                        0 => m.top = *v,
                        1 => m.right = *v,
                        2 => m.bottom = *v,
                        _ => m.left = *v,
                    }
                }
            }
        };
        let o = &self.margin_overrides;
        apply(if page_number % 2 == 1 {
            &o.right
        } else {
            &o.left
        });
        if page_number == 1 {
            apply(&o.first);
        }
        m
    }

    /// Content height of page `page_number` (1-based), in pt.
    pub fn content_height_for(&self, page_number: u32) -> f32 {
        let (_, h) = self.size.dimensions_pt();
        let m = self.margins_for(page_number);
        (h - m.top - m.bottom).max(0.0)
    }

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

#[cfg(test)]
mod margin_tests {
    use super::*;

    fn mm(m: &PageMargins) -> [i32; 4] {
        [m.top, m.right, m.bottom, m.left].map(|v| (v / MM_TO_PT).round() as i32)
    }

    #[test]
    fn margin_shorthand() {
        assert_eq!(mm(&PageMargins::from_css_str("15mm")), [15, 15, 15, 15]);
        assert_eq!(
            mm(&PageMargins::from_css_str("10mm 20mm")),
            [10, 20, 10, 20]
        );
        assert_eq!(
            mm(&PageMargins::from_css_str("30mm 18mm 20mm")),
            [30, 18, 20, 18]
        );
        assert_eq!(
            mm(&PageMargins::from_css_str("1cm 2cm 3cm 4cm")),
            [10, 20, 30, 40]
        );
        assert_eq!(mm(&PageMargins::from_css_str("0")), [0, 0, 0, 0]);
        assert_eq!(mm(&PageMargins::from_css_str("bogus")), [20, 20, 20, 20]);
    }
}

#[cfg(test)]
mod per_page_margin_tests {
    use super::*;

    #[test]
    fn first_beats_left_right_beats_base() {
        let mut c = PageConfig {
            margins: PageMargins::uniform_pt(50.0),
            ..Default::default()
        };
        c.margin_overrides.right[3] = Some(80.0);
        c.margin_overrides.left[3] = Some(30.0);
        c.margin_overrides.first[0] = Some(200.0);
        c.margin_overrides.first[3] = Some(10.0);
        let p1 = c.margins_for(1);
        assert_eq!((p1.top, p1.left), (200.0, 10.0));
        let p2 = c.margins_for(2);
        assert_eq!((p2.top, p2.left), (50.0, 30.0));
        let p3 = c.margins_for(3);
        assert_eq!((p3.top, p3.left), (50.0, 80.0));
        assert!(c.content_height_for(1) < c.content_height_for(2));
    }

    #[test]
    fn strict_shorthand() {
        assert_eq!(
            PageMargins::parse_shorthand("1in 2pt"),
            Some([72.0, 2.0, 72.0, 2.0])
        );
        assert_eq!(PageMargins::parse_shorthand("auto"), None);
    }
}
