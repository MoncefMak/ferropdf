//! Paint operations — converts layout boxes into PDF drawing commands.

use crate::css::properties::CssProperty;
use crate::css::values::Color;
use crate::fonts::FontCache;
use crate::images::ImageCache;
use crate::layout::box_model::{LayoutBox, LayoutBoxType};
use crate::layout::pagination::Page;

/// A paint command to be executed by the PDF generator.
#[derive(Debug, Clone)]
pub enum PaintCommand {
    /// Draw a filled rectangle.
    FillRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Color,
    },
    /// Draw a rectangle border.
    StrokeRect {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        color: Color,
        width_px: f64,
    },
    /// Draw text.
    Text {
        x: f64,
        y: f64,
        text: String,
        font_family: String,
        font_size: f64,
        font_weight: u32,
        italic: bool,
        color: Color,
        align: TextAlign,
        available_width: f64,
        direction_rtl: bool,
    },
    /// Draw an image.
    Image {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        src: String,
    },
    /// Draw a line.
    Line {
        x1: f64,
        y1: f64,
        x2: f64,
        y2: f64,
        color: Color,
        width_px: f64,
    },
    /// Set a link/anchor.
    Link {
        x: f64,
        y: f64,
        width: f64,
        height: f64,
        url: String,
    },
}

#[derive(Debug, Clone, Copy)]
pub enum TextAlign {
    Left,
    Center,
    Right,
    Justify,
}

/// The renderer converts layout pages into paint commands.
pub struct Renderer {
    _font_cache: FontCache,
    _image_cache: ImageCache,
}

impl Renderer {
    pub fn new() -> Self {
        Self {
            _font_cache: FontCache::default(),
            _image_cache: ImageCache::default(),
        }
    }

    pub fn with_caches(font_cache: FontCache, image_cache: ImageCache) -> Self {
        Self {
            _font_cache: font_cache,
            _image_cache: image_cache,
        }
    }

    /// Render all pages into paint commands.
    pub fn render_pages(&self, pages: &[Page]) -> Vec<Vec<PaintCommand>> {
        pages
            .iter()
            .map(|page| self.render_page(page))
            .collect()
    }

    /// Render a single page into paint commands.
    fn render_page(&self, page: &Page) -> Vec<PaintCommand> {
        let mut commands = Vec::new();

        // White background
        commands.push(PaintCommand::FillRect {
            x: 0.0,
            y: 0.0,
            width: page.layout.size.width,
            height: page.layout.size.height,
            color: Color::white(),
        });

        // Render content
        let offset_x = page.layout.content_left();
        let offset_y = page.layout.content_top();

        for content_box in &page.content {
            self.render_box(content_box, offset_x, offset_y, &mut commands);
        }

        commands
    }

    /// Render a single layout box and its children.
    fn render_box(
        &self,
        layout_box: &LayoutBox,
        offset_x: f64,
        offset_y: f64,
        commands: &mut Vec<PaintCommand>,
    ) {
        let dims = &layout_box.dimensions;
        let border_box = dims.border_box();
        let x = border_box.x + offset_x;
        let y = border_box.y + offset_y;
        let w = border_box.width;
        let h = border_box.height;

        // Background
        let bg_color = layout_box.style.background_color();
        if bg_color.a > 0.0 {
            commands.push(PaintCommand::FillRect {
                x,
                y,
                width: w,
                height: h,
                color: bg_color,
            });
        }

        // Borders
        self.render_borders(layout_box, x, y, w, h, commands);

        // Text content
        if let Some(text) = &layout_box.text {
            if !text.is_empty() {
                let font_size = layout_box
                    .style
                    .font_size_px(16.0, 16.0);
                let font_weight = layout_box.style.font_weight();
                let font_family = layout_box.style.font_family().to_string();
                let color = layout_box.style.color();
                let italic = layout_box
                    .style
                    .get(&CssProperty::FontStyle)
                    .map(|v| v.to_string() == "italic")
                    .unwrap_or(false);

                let align = match layout_box.style.text_align() {
                    "center" => TextAlign::Center,
                    "right" => TextAlign::Right,
                    "justify" => TextAlign::Justify,
                    _ => TextAlign::Left,
                };

                let direction_rtl = layout_box.style.is_rtl();

                let text_x = dims.content.x + offset_x;
                let text_y = dims.content.y + offset_y;

                commands.push(PaintCommand::Text {
                    x: text_x,
                    y: text_y,
                    text: text.clone(),
                    font_family,
                    font_size,
                    font_weight,
                    italic,
                    color,
                    align,
                    available_width: dims.content.width,
                    direction_rtl,
                });
            }
        }

        // Image
        if let (Some(src), LayoutBoxType::Replaced) = (&layout_box.image_src, &layout_box.box_type) {
            commands.push(PaintCommand::Image {
                x: dims.content.x + offset_x,
                y: dims.content.y + offset_y,
                width: dims.content.width,
                height: dims.content.height,
                src: src.clone(),
            });
        }

        // Link
        if let Some(href) = layout_box.attributes.get("href") {
            if layout_box.tag_name.as_deref() == Some("a") {
                commands.push(PaintCommand::Link {
                    x,
                    y,
                    width: w,
                    height: h,
                    url: href.clone(),
                });
            }
        }

        // Render children
        let child_offset_x = offset_x + dims.content.x;
        let child_offset_y = offset_y + dims.content.y;

        for child in &layout_box.children {
            self.render_box(child, child_offset_x, child_offset_y, commands);
        }
    }

    fn render_borders(
        &self,
        layout_box: &LayoutBox,
        x: f64,
        y: f64,
        w: f64,
        h: f64,
        commands: &mut Vec<PaintCommand>,
    ) {
        let border = &layout_box.dimensions.border;

        // Top border
        if border.top > 0.0 {
            let color = layout_box
                .style
                .get(&CssProperty::BorderTopColor)
                .and_then(|v| v.as_color())
                .unwrap_or(Color::black());

            commands.push(PaintCommand::Line {
                x1: x,
                y1: y,
                x2: x + w,
                y2: y,
                color,
                width_px: border.top,
            });
        }

        // Right border
        if border.right > 0.0 {
            let color = layout_box
                .style
                .get(&CssProperty::BorderRightColor)
                .and_then(|v| v.as_color())
                .unwrap_or(Color::black());

            commands.push(PaintCommand::Line {
                x1: x + w,
                y1: y,
                x2: x + w,
                y2: y + h,
                color,
                width_px: border.right,
            });
        }

        // Bottom border
        if border.bottom > 0.0 {
            let color = layout_box
                .style
                .get(&CssProperty::BorderBottomColor)
                .and_then(|v| v.as_color())
                .unwrap_or(Color::black());

            commands.push(PaintCommand::Line {
                x1: x,
                y1: y + h,
                x2: x + w,
                y2: y + h,
                color,
                width_px: border.bottom,
            });
        }

        // Left border
        if border.left > 0.0 {
            let color = layout_box
                .style
                .get(&CssProperty::BorderLeftColor)
                .and_then(|v| v.as_color())
                .unwrap_or(Color::black());

            commands.push(PaintCommand::Line {
                x1: x,
                y1: y,
                x2: x,
                y2: y + h,
                color,
                width_px: border.left,
            });
        }
    }
}

impl Default for Renderer {
    fn default() -> Self {
        Self::new()
    }
}
