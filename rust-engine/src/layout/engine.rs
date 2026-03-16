//! The main layout engine — builds a layout tree from a DOM + stylesheet.

use std::sync::Arc;

use unicode_bidi::BidiInfo;

use crate::css::properties::{ComputedStyle, CssProperty};
use crate::css::stylesheet::{default_stylesheet, Stylesheet};
use crate::css::values::CssValue;
use crate::error::Result;
use crate::fonts::metrics;
use crate::fonts::shaping;
use crate::fonts::FontCache;
use crate::html::dom::{DomNode, DomTree, ElementData, NodeType};
use crate::pdf::writer;

use super::box_model::{EdgeSizes, LayoutBox, LayoutBoxType};
use super::pagination::{Page, PageLayout, Paginator};
use super::style_resolver::StyleResolver;
use super::table_layout;

/// Detect whether a text string is predominantly RTL using Unicode Bidi algorithm.
fn is_text_rtl(text: &str) -> bool {
    let bidi = BidiInfo::new(text, None);
    bidi.paragraphs
        .first()
        .map(|p| p.level.is_rtl())
        .unwrap_or(false)
}

/// The layout engine that computes positions and sizes for all elements.
pub struct LayoutEngine {
    page_layout: PageLayout,
    root_font_size: f64,
    font_cache: Option<Arc<FontCache>>,
}

impl LayoutEngine {
    pub fn new(page_layout: PageLayout) -> Self {
        Self {
            page_layout,
            root_font_size: 16.0,
            font_cache: None,
        }
    }

    pub fn with_font_cache(mut self, cache: Arc<FontCache>) -> Self {
        self.font_cache = Some(cache);
        self
    }

    /// Measure text width in px, using TTF metrics from FontCache if available,
    /// otherwise falling back to built-in font metrics.
    fn measure_text_width(
        &self,
        text: &str,
        font_family: &str,
        font_weight: u32,
        italic: bool,
        font_size: f64,
    ) -> f64 {
        if let Some(ref cache) = self.font_cache {
            if let Some(font_data) = cache.get_font(font_family, font_weight, italic) {
                if !font_data.data.is_empty() {
                    return shaping::measure_ttf_text_width_px(&font_data.data, text, font_size);
                }
            }
        }
        let builtin_name = writer::resolve_builtin_font_name(font_family, font_weight, italic);
        metrics::measure_text_width_px(text, builtin_name, font_size)
    }

    /// Perform full layout: HTML + CSS → paginated layout boxes.
    pub fn layout(&self, dom: &DomTree, stylesheets: &[Stylesheet]) -> Result<Vec<Page>> {
        // Build combined stylesheets (default + user)
        let defaults = default_stylesheet();
        let mut all_sheets: Vec<&Stylesheet> = vec![&defaults];
        for sheet in stylesheets {
            all_sheets.push(sheet);
        }

        let resolver = StyleResolver::new(all_sheets);

        // Find the body element (or use root)
        let body = dom.body().unwrap_or(&dom.root);

        // Build layout tree
        let root_style = ComputedStyle::default_root();
        let mut root = self.build_layout_tree(body, &root_style, &resolver, &[], &[], &[])?;

        // Compute dimensions
        let container_width = self.page_layout.content_width();
        self.compute_layout(&mut root, container_width);

        // Paginate
        let paginator = Paginator::new(self.page_layout.clone());
        let pages = paginator.paginate(&root);

        Ok(pages)
    }

    /// Build a layout tree from the DOM.
    fn build_layout_tree(
        &self,
        node: &DomNode,
        parent_style: &ComputedStyle,
        resolver: &StyleResolver,
        ancestors: &[&ElementData],
        siblings: &[&ElementData],
        following_siblings: &[&ElementData],
    ) -> Result<LayoutBox> {
        match &node.node_type {
            NodeType::Text(text) => {
                // Collapse whitespace runs to single space, preserve leading/trailing
                let has_leading = text.starts_with(|c: char| c.is_whitespace());
                let has_trailing = text.ends_with(|c: char| c.is_whitespace());
                let words: Vec<&str> = text.split_whitespace().collect();
                if words.is_empty() {
                    // Pure whitespace node — keep as a single space so IFC knows
                    // there was whitespace between siblings
                    if has_leading || has_trailing {
                        return Ok(LayoutBox::text_box(" ".to_string(), parent_style.clone()));
                    }
                    return Ok(LayoutBox::text_box(String::new(), parent_style.clone()));
                }
                let mut collapsed = String::new();
                if has_leading {
                    collapsed.push(' ');
                }
                collapsed.push_str(&words.join(" "));
                if has_trailing {
                    collapsed.push(' ');
                }
                Ok(LayoutBox::text_box(collapsed, parent_style.clone()))
            }
            NodeType::Element(data) => {
                // Compute style
                let mut style = resolver.compute_style(
                    data,
                    parent_style,
                    ancestors,
                    siblings,
                    following_siblings,
                );
                StyleResolver::apply_tag_defaults(&data.tag_name, &mut style);

                // Check if hidden
                if style.is_hidden() {
                    return Ok(LayoutBox::new(LayoutBoxType::Block, style));
                }

                // Determine box type
                let box_type = match style.display() {
                    "inline" => LayoutBoxType::Inline,
                    "inline-block" => LayoutBoxType::InlineBlock,
                    "flex" => LayoutBoxType::Flex,
                    "table" => LayoutBoxType::Table,
                    "table-row" => LayoutBoxType::TableRow,
                    "table-cell" => LayoutBoxType::TableCell,
                    "list-item" => LayoutBoxType::ListItem,
                    _ => LayoutBoxType::Block,
                };

                let mut layout_box = LayoutBox::new(box_type, style.clone());
                layout_box.tag_name = Some(data.tag_name.clone());
                layout_box.attributes = data.attributes.clone();
                layout_box.page_break_before = style.has_page_break_before();
                layout_box.page_break_after = style.has_page_break_after();
                layout_box.avoid_break_inside = style.avoid_page_break_inside();

                // Handle image src
                if data.tag_name == "img" {
                    layout_box.image_src = data.get_attr("src").map(|s| s.to_string());
                    layout_box.box_type = LayoutBoxType::Replaced;
                }

                // Build children
                let mut child_ancestors = ancestors.to_vec();
                child_ancestors.push(data);

                // Pre-collect element children so we can compute following siblings
                let element_children: Vec<&ElementData> = node
                    .children
                    .iter()
                    .filter_map(|c| {
                        if let NodeType::Element(d) = &c.node_type {
                            Some(d)
                        } else {
                            None
                        }
                    })
                    .collect();

                let mut prev_siblings: Vec<&ElementData> = Vec::new();

                for (node_idx, child) in node.children.iter().enumerate() {
                    // Compute following element-siblings for this child
                    let following: Vec<&ElementData> = node.children[node_idx + 1..]
                        .iter()
                        .filter_map(|c| {
                            if let NodeType::Element(d) = &c.node_type {
                                Some(d)
                            } else {
                                None
                            }
                        })
                        .collect();

                    let child_box = self.build_layout_tree(
                        child,
                        &style,
                        resolver,
                        &child_ancestors,
                        &prev_siblings,
                        &following,
                    )?;

                    // Track element siblings for selector matching
                    if let NodeType::Element(child_data) = &child.node_type {
                        prev_siblings.push(child_data);
                    }

                    // Suppress unused variable warning
                    let _ = &element_children;

                    layout_box.children.push(child_box);
                }

                Ok(layout_box)
            }
            NodeType::Comment(_) | NodeType::Document => {
                // Process children of document nodes
                let mut layout_box = LayoutBox::new(LayoutBoxType::Block, parent_style.clone());
                for (node_idx, child) in node.children.iter().enumerate() {
                    let following: Vec<&ElementData> = node.children[node_idx + 1..]
                        .iter()
                        .filter_map(|c| {
                            if let NodeType::Element(d) = &c.node_type {
                                Some(d)
                            } else {
                                None
                            }
                        })
                        .collect();
                    let child_box = self.build_layout_tree(
                        child,
                        parent_style,
                        resolver,
                        ancestors,
                        siblings,
                        &following,
                    )?;
                    layout_box.children.push(child_box);
                }
                Ok(layout_box)
            }
        }
    }

    /// Compute dimensions for the layout tree.
    fn compute_layout(&self, layout_box: &mut LayoutBox, container_width: f64) {
        let font_size = layout_box
            .style
            .font_size_px(self.root_font_size, self.root_font_size);

        // Compute margins, padding, borders
        self.compute_box_model(layout_box, container_width, font_size);

        match &layout_box.box_type {
            LayoutBoxType::Block | LayoutBoxType::ListItem | LayoutBoxType::AnonymousBlock => {
                self.layout_block(layout_box, container_width, font_size);
            }
            LayoutBoxType::Inline | LayoutBoxType::AnonymousInline => {
                self.layout_inline(layout_box, font_size);
            }
            LayoutBoxType::InlineBlock => {
                self.layout_inline_block(layout_box, container_width, font_size);
            }
            LayoutBoxType::Flex => {
                self.layout_flex(layout_box, container_width, font_size);
            }
            LayoutBoxType::Table => {
                // Determine border-collapse mode
                let collapsed = layout_box
                    .style
                    .get(&CssProperty::BorderCollapse)
                    .map(|v| v.to_string() == "collapse")
                    .unwrap_or(false);

                // Pre-compute box models (padding/border) for all rows and cells
                // BEFORE the table layout algorithm uses them.
                self.precompute_table_box_models(layout_box, container_width);

                table_layout::layout_table(
                    layout_box,
                    container_width,
                    font_size,
                    self.root_font_size,
                    collapsed,
                );
                // Now layout cell contents so text renders correctly
                self.layout_table_cell_contents(layout_box, font_size);
            }
            LayoutBoxType::Replaced => {
                self.layout_replaced(layout_box, container_width, font_size);
            }
            _ => {
                self.layout_block(layout_box, container_width, font_size);
            }
        }

        // S3-3: Apply min/max size constraints after layout
        self.apply_size_constraints(layout_box, container_width, font_size);

        // S3-4: Apply position: relative offset (visual-only, stays in flow)
        if layout_box
            .style
            .get(&CssProperty::Position)
            .map(|v| matches!(v, CssValue::Keyword(s) if s == "relative"))
            .unwrap_or(false)
        {
            if let Some(top) = layout_box
                .style
                .get(&CssProperty::Top)
                .and_then(|v| v.as_px(0.0, font_size, self.root_font_size))
            {
                layout_box.dimensions.content.y += top;
            }
            if let Some(left) = layout_box
                .style
                .get(&CssProperty::Left)
                .and_then(|v| v.as_px(container_width, font_size, self.root_font_size))
            {
                layout_box.dimensions.content.x += left;
            }
            if let Some(right) = layout_box
                .style
                .get(&CssProperty::Right)
                .and_then(|v| v.as_px(container_width, font_size, self.root_font_size))
            {
                layout_box.dimensions.content.x -= right;
            }
            if let Some(bottom) = layout_box
                .style
                .get(&CssProperty::Bottom)
                .and_then(|v| v.as_px(0.0, font_size, self.root_font_size))
            {
                layout_box.dimensions.content.y -= bottom;
            }
        }
    }

    /// Compute margins, padding, and border from the computed style.
    fn compute_box_model(&self, layout_box: &mut LayoutBox, container_width: f64, font_size: f64) {
        let style = &layout_box.style;

        layout_box.dimensions.margin = EdgeSizes::new(
            style.get_length(
                &CssProperty::MarginTop,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::MarginRight,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::MarginBottom,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::MarginLeft,
                container_width,
                font_size,
                self.root_font_size,
            ),
        );

        layout_box.dimensions.padding = EdgeSizes::new(
            style.get_length(
                &CssProperty::PaddingTop,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::PaddingRight,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::PaddingBottom,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::PaddingLeft,
                container_width,
                font_size,
                self.root_font_size,
            ),
        );

        layout_box.dimensions.border = EdgeSizes::new(
            style.get_length(
                &CssProperty::BorderTopWidth,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::BorderRightWidth,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::BorderBottomWidth,
                container_width,
                font_size,
                self.root_font_size,
            ),
            style.get_length(
                &CssProperty::BorderLeftWidth,
                container_width,
                font_size,
                self.root_font_size,
            ),
        );
    }

    /// Layout a block-level box.
    fn layout_block(&self, layout_box: &mut LayoutBox, container_width: f64, font_size: f64) {
        // Width calculation
        let explicit_width = layout_box
            .style
            .get(&CssProperty::Width)
            .and_then(|v| v.as_px(container_width, font_size, self.root_font_size));

        let content_width = explicit_width.unwrap_or(
            container_width
                - layout_box.dimensions.margin.horizontal()
                - layout_box.dimensions.padding.horizontal()
                - layout_box.dimensions.border.horizontal(),
        );

        layout_box.dimensions.content.width = content_width.max(0.0);

        // Determine if this is an inline formatting context
        let all_inline = !layout_box.children.is_empty()
            && layout_box.children.iter().all(|c| {
                matches!(
                    c.box_type,
                    LayoutBoxType::Inline
                        | LayoutBoxType::AnonymousInline
                        | LayoutBoxType::InlineBlock
                )
            });

        let y = if all_inline {
            self.layout_inline_formatting_context(layout_box, content_width, font_size)
        } else {
            // Block formatting context — stack children vertically
            let mut y = 0.0;
            for child in &mut layout_box.children {
                self.compute_layout(child, layout_box.dimensions.content.width);
                child.dimensions.content.y = y + child.dimensions.margin.top;
                child.dimensions.content.x = child.dimensions.margin.left
                    + child.dimensions.padding.left
                    + child.dimensions.border.left;

                y += child.dimensions.margin_box().height;
            }
            y
        };

        // Height calculation
        let explicit_height = layout_box
            .style
            .get(&CssProperty::Height)
            .and_then(|v| v.as_px(0.0, font_size, self.root_font_size));

        layout_box.dimensions.content.height = explicit_height.unwrap_or(y);
    }

    /// Layout inline children using an inline formatting context (line boxes).
    ///
    /// Text nodes are split into individual words for proper word-wrapping.
    /// Non-text inline elements (strong, em, etc.) are placed as atomic units.
    /// Returns the total height of all line boxes.
    fn layout_inline_formatting_context(
        &self,
        layout_box: &mut LayoutBox,
        content_width: f64,
        _font_size: f64,
    ) -> f64 {
        // First compute the intrinsic size of each inline child
        for child in &mut layout_box.children {
            self.compute_layout(child, content_width);
        }

        // Build a list of placeable items: split text nodes into per-word
        // boxes, keep non-text inline elements (strong, em, span, etc.) as
        // atomic units.  Track whether a space is needed before each item.
        let old_children = std::mem::take(&mut layout_box.children);
        let mut items: Vec<(LayoutBox, bool, f64)> = Vec::new(); // (box, needs_space, space_width)
        let mut trailing_space = false;

        for child in old_children {
            if let Some(ref text) = child.text {
                if text.is_empty() {
                    continue;
                }
                // Pure whitespace node
                if text.trim().is_empty() {
                    trailing_space = true;
                    continue;
                }

                let fs = child
                    .style
                    .font_size_px(self.root_font_size, self.root_font_size);
                let family = child.style.font_family();
                let weight = child.style.font_weight();
                let italic = child
                    .style
                    .get(&CssProperty::FontStyle)
                    .map(|v| matches!(v, CssValue::Keyword(s) if s == "italic" || s == "oblique"))
                    .unwrap_or(false);
                let lh = child.style.line_height(fs);
                let space_w = self.measure_text_width(" ", family, weight, italic, fs);

                let has_leading = text.starts_with(' ');
                let has_trailing = text.ends_with(' ');

                let text_words: Vec<&str> = text.split_whitespace().collect();
                if text_words.is_empty() {
                    if has_leading || has_trailing {
                        trailing_space = true;
                    }
                    continue;
                }

                for (i, word) in text_words.iter().enumerate() {
                    let word_w = self.measure_text_width(word, family, weight, italic, fs);
                    let mut wb = LayoutBox::text_box(word.to_string(), child.style.clone());
                    wb.dimensions.content.width = word_w;
                    wb.dimensions.content.height = lh;

                    let needs_space = if i == 0 {
                        has_leading || trailing_space
                    } else {
                        true
                    };

                    items.push((wb, needs_space, space_w));
                }

                trailing_space = has_trailing;
            } else {
                // Non-text inline element — place as atomic unit
                let advance = child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
                    + child.dimensions.content.width
                    + child.dimensions.padding.right
                    + child.dimensions.border.right
                    + child.dimensions.margin.right;

                let fs = child
                    .style
                    .font_size_px(self.root_font_size, self.root_font_size);
                let family = child.style.font_family();
                let weight = child.style.font_weight();
                let italic = child
                    .style
                    .get(&CssProperty::FontStyle)
                    .map(|v| matches!(v, CssValue::Keyword(s) if s == "italic" || s == "oblique"))
                    .unwrap_or(false);
                let space_w = self.measure_text_width(" ", family, weight, italic, fs);

                // If the element has zero width (e.g. empty <br>), skip
                if advance <= 0.0 && child.children.is_empty() {
                    continue;
                }

                let needs_space = trailing_space;
                items.push((child, needs_space, space_w));
                trailing_space = false;
            }
        }

        // Place items on lines with word-wrapping
        let text_align = layout_box.style.text_align();
        let mut new_children: Vec<LayoutBox> = Vec::new();
        let mut x: f64 = 0.0;
        let mut y: f64 = 0.0;
        let mut line_height: f64 = 0.0;
        let mut line_start_idx: usize = 0;
        let mut line_infos: Vec<(usize, usize, f64, f64)> = Vec::new(); // (start, end, width, height)

        for (mut item_box, needs_space, space_w) in items {
            let advance = item_box.dimensions.margin.left
                + item_box.dimensions.border.left
                + item_box.dimensions.padding.left
                + item_box.dimensions.content.width
                + item_box.dimensions.padding.right
                + item_box.dimensions.border.right
                + item_box.dimensions.margin.right;

            let item_height = item_box.dimensions.margin.top
                + item_box.dimensions.border.top
                + item_box.dimensions.padding.top
                + item_box.dimensions.content.height
                + item_box.dimensions.padding.bottom
                + item_box.dimensions.border.bottom
                + item_box.dimensions.margin.bottom;

            let space = if needs_space && x > 0.0 { space_w } else { 0.0 };

            // Wrap to next line if needed
            if x > 0.0 && x + space + advance > content_width {
                line_infos.push((line_start_idx, new_children.len(), x, line_height));
                y += line_height;
                x = 0.0;
                line_height = 0.0;
                line_start_idx = new_children.len();
            } else {
                x += space;
            }

            // Position the item's content area
            item_box.dimensions.content.x = x
                + item_box.dimensions.margin.left
                + item_box.dimensions.border.left
                + item_box.dimensions.padding.left;
            item_box.dimensions.content.y = y
                + item_box.dimensions.margin.top
                + item_box.dimensions.border.top
                + item_box.dimensions.padding.top;

            x += advance;
            line_height = line_height.max(item_height);

            new_children.push(item_box);
        }

        // Record last line
        if line_start_idx < new_children.len() {
            line_infos.push((line_start_idx, new_children.len(), x, line_height));
        }

        // For RTL direction, mirror item positions within each line.
        // Detect RTL from explicit CSS `direction: rtl` or auto-detect from
        // text content using the Unicode Bidirectional Algorithm.
        let is_rtl = if layout_box.style.is_rtl() {
            true
        } else {
            // Auto-detect: collect all text content and check Unicode bidi
            let combined_text: String = new_children
                .iter()
                .filter_map(|c| c.text.as_deref())
                .collect::<Vec<_>>()
                .join(" ");
            !combined_text.is_empty() && is_text_rtl(&combined_text)
        };
        if is_rtl {
            for &(start, end, line_width, _) in &line_infos {
                for child in new_children.iter_mut().take(end).skip(start) {
                    let dims = &child.dimensions;
                    let outer_left =
                        dims.content.x - dims.margin.left - dims.border.left - dims.padding.left;
                    let outer_width = dims.margin.left
                        + dims.border.left
                        + dims.padding.left
                        + dims.content.width
                        + dims.padding.right
                        + dims.border.right
                        + dims.margin.right;
                    let new_outer_left = line_width - outer_left - outer_width;
                    child.dimensions.content.x =
                        new_outer_left + dims.margin.left + dims.border.left + dims.padding.left;
                }
            }
        }

        // Apply text-align
        match text_align {
            "center" => {
                for &(start, end, width, _) in &line_infos {
                    let offset = (content_width - width) / 2.0;
                    if offset > 0.0 {
                        for child in new_children.iter_mut().take(end).skip(start) {
                            child.dimensions.content.x += offset;
                        }
                    }
                }
            }
            "right" => {
                for &(start, end, width, _) in &line_infos {
                    let offset = content_width - width;
                    if offset > 0.0 {
                        for child in new_children.iter_mut().take(end).skip(start) {
                            child.dimensions.content.x += offset;
                        }
                    }
                }
            }
            _ => {} // left is default
        }

        layout_box.children = new_children;
        y + line_height
    }

    /// Layout an inline element.
    ///
    /// Handles both text leaf nodes and inline elements that wrap children
    /// (e.g. `<strong>`, `<em>`, `<a>`).
    fn layout_inline(&self, layout_box: &mut LayoutBox, font_size: f64) {
        if let Some(text) = &layout_box.text {
            // ── Leaf text node ──
            if text.is_empty() {
                return;
            }

            let family = layout_box.style.font_family();
            let weight = layout_box.style.font_weight();
            let italic = layout_box
                .style
                .get(&CssProperty::FontStyle)
                .map(|v| matches!(v, CssValue::Keyword(s) if s == "italic" || s == "oblique"))
                .unwrap_or(false);

            let text_width = self.measure_text_width(text, family, weight, italic, font_size);
            let line_height = layout_box.style.line_height(font_size);

            layout_box.dimensions.content.width = text_width;
            layout_box.dimensions.content.height = line_height;
        } else if !layout_box.children.is_empty() {
            // ── Inline element wrapping children (e.g. <strong>, <em>) ──
            // Layout children inline and sum up their widths.
            let container_width = f64::MAX; // unconstrained for intrinsic measurement

            for child in &mut layout_box.children {
                self.compute_layout(child, container_width);
            }

            // Place children left-to-right and compute total width
            let mut x = 0.0;
            let mut max_height: f64 = 0.0;

            for child in &mut layout_box.children {
                let advance = child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left
                    + child.dimensions.content.width
                    + child.dimensions.padding.right
                    + child.dimensions.border.right
                    + child.dimensions.margin.right;

                let child_height = child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top
                    + child.dimensions.content.height
                    + child.dimensions.padding.bottom
                    + child.dimensions.border.bottom
                    + child.dimensions.margin.bottom;

                child.dimensions.content.x = x
                    + child.dimensions.margin.left
                    + child.dimensions.border.left
                    + child.dimensions.padding.left;
                child.dimensions.content.y = child.dimensions.margin.top
                    + child.dimensions.border.top
                    + child.dimensions.padding.top;

                x += advance;
                max_height = max_height.max(child_height);
            }

            layout_box.dimensions.content.width = x;
            layout_box.dimensions.content.height = max_height;
        }
    }

    /// Layout an inline-block element.
    fn layout_inline_block(
        &self,
        layout_box: &mut LayoutBox,
        container_width: f64,
        font_size: f64,
    ) {
        self.layout_block(layout_box, container_width, font_size);
    }

    /// Layout a flex container.
    fn layout_flex(&self, layout_box: &mut LayoutBox, container_width: f64, font_size: f64) {
        // Set content width (like layout_block)
        let explicit_width = layout_box
            .style
            .get(&CssProperty::Width)
            .and_then(|v| v.as_px(container_width, font_size, self.root_font_size));

        let content_width = explicit_width
            .unwrap_or(
                container_width
                    - layout_box.dimensions.margin.horizontal()
                    - layout_box.dimensions.padding.horizontal()
                    - layout_box.dimensions.border.horizontal(),
            )
            .max(0.0);

        layout_box.dimensions.content.width = content_width;

        let flex_direction = layout_box
            .style
            .get(&CssProperty::FlexDirection)
            .and_then(|v| match v {
                CssValue::Keyword(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "row".to_string());

        let justify_content = layout_box
            .style
            .get(&CssProperty::JustifyContent)
            .and_then(|v| match v {
                CssValue::Keyword(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "flex-start".to_string());

        let flex_wrap = layout_box
            .style
            .get(&CssProperty::FlexWrap)
            .and_then(|v| match v {
                CssValue::Keyword(s) => Some(s.clone()),
                _ => None,
            })
            .unwrap_or_else(|| "nowrap".to_string());

        // Layout children to get their natural sizes
        for child in &mut layout_box.children {
            self.compute_layout(child, content_width);
        }

        let gap = layout_box.style.get_length(
            &CssProperty::Gap,
            container_width,
            font_size,
            self.root_font_size,
        );

        match flex_direction.as_str() {
            "column" | "column-reverse" => {
                // Vertical flex layout
                let mut y = 0.0;
                let num_children = layout_box.children.len();
                let iter: Box<dyn Iterator<Item = &mut LayoutBox>> =
                    if flex_direction == "column-reverse" {
                        Box::new(layout_box.children.iter_mut().rev())
                    } else {
                        Box::new(layout_box.children.iter_mut())
                    };

                for (i, child) in iter.enumerate() {
                    child.dimensions.content.x = child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left;
                    child.dimensions.content.y = y
                        + child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top;
                    y += child.dimensions.margin_box().height;
                    if i < num_children.saturating_sub(1) {
                        y += gap;
                    }
                }
                layout_box.dimensions.content.height = y;
            }
            _ => {
                // Horizontal flex layout (row)

                // === STEP 1: Apply flex-basis to override natural sizes ===
                for child in layout_box.children.iter_mut() {
                    let basis = child
                        .style
                        .get(&CssProperty::FlexBasis)
                        .and_then(|v| match v {
                            CssValue::Length(l) => {
                                Some(l.to_px(content_width, font_size, self.root_font_size))
                            }
                            CssValue::Calc(expr) => {
                                Some(expr.to_px(content_width, font_size, self.root_font_size))
                            }
                            CssValue::Auto | CssValue::Keyword(_) => None,
                            _ => None,
                        });
                    if let Some(b) = basis {
                        child.dimensions.content.width = (b
                            - child.dimensions.padding.horizontal()
                            - child.dimensions.border.horizontal())
                        .max(0.0);
                    }
                }

                // === STEP 2: Compute free space ===
                let num_children = layout_box.children.len();
                let total_gap = gap * num_children.saturating_sub(1) as f64;
                let total_base_width: f64 = layout_box
                    .children
                    .iter()
                    .map(|c| c.dimensions.margin_box().width)
                    .sum();
                let free_space = content_width - total_base_width - total_gap;

                if free_space >= 0.0 {
                    // === STEP 3a: Distribute free space via flex-grow ===
                    let total_grow: f64 = layout_box
                        .children
                        .iter()
                        .map(|c| {
                            c.style
                                .get(&CssProperty::FlexGrow)
                                .and_then(|v| {
                                    if let CssValue::Number(n) = v {
                                        Some(*n)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0.0)
                        })
                        .sum();
                    if total_grow > 0.0 {
                        for child in layout_box.children.iter_mut() {
                            let grow = child
                                .style
                                .get(&CssProperty::FlexGrow)
                                .and_then(|v| {
                                    if let CssValue::Number(n) = v {
                                        Some(*n)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(0.0);
                            child.dimensions.content.width += free_space * (grow / total_grow);
                        }
                    }
                } else {
                    // === STEP 3b: Reduce via flex-shrink ===
                    let overflow = -free_space;
                    let total_shrink_factor: f64 = layout_box
                        .children
                        .iter()
                        .map(|c| {
                            let shrink = c
                                .style
                                .get(&CssProperty::FlexShrink)
                                .and_then(|v| {
                                    if let CssValue::Number(n) = v {
                                        Some(*n)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(1.0);
                            let basis = c.dimensions.content.width
                                + c.dimensions.padding.horizontal()
                                + c.dimensions.border.horizontal();
                            shrink * basis
                        })
                        .sum();
                    if total_shrink_factor > 0.0 {
                        for child in layout_box.children.iter_mut() {
                            let shrink = child
                                .style
                                .get(&CssProperty::FlexShrink)
                                .and_then(|v| {
                                    if let CssValue::Number(n) = v {
                                        Some(*n)
                                    } else {
                                        None
                                    }
                                })
                                .unwrap_or(1.0);
                            let basis = child.dimensions.content.width
                                + child.dimensions.padding.horizontal()
                                + child.dimensions.border.horizontal();
                            let reduction = overflow * (shrink * basis / total_shrink_factor);
                            child.dimensions.content.width =
                                (child.dimensions.content.width - reduction).max(0.0);
                        }
                    }
                }

                // Recompute remaining space for justify-content (after grow/shrink)
                let total_child_width: f64 = layout_box
                    .children
                    .iter()
                    .map(|c| c.dimensions.margin_box().width)
                    .sum();
                let remaining_space = (content_width - total_child_width - total_gap).max(0.0);

                // Handle flex-wrap: wrap — split children into rows
                let wrapping = flex_wrap == "wrap" || flex_wrap == "wrap-reverse";

                if wrapping {
                    // Wrap children into multiple rows
                    let mut rows: Vec<Vec<usize>> = Vec::new();
                    let mut current_row: Vec<usize> = Vec::new();
                    let mut row_x: f64 = 0.0;

                    for (i, child) in layout_box.children.iter().enumerate() {
                        let child_w = child.dimensions.margin_box().width;
                        if row_x > 0.0 && row_x + gap + child_w > content_width {
                            rows.push(std::mem::take(&mut current_row));
                            row_x = 0.0;
                        }
                        current_row.push(i);
                        if row_x > 0.0 {
                            row_x += gap;
                        }
                        row_x += child_w;
                    }
                    if !current_row.is_empty() {
                        rows.push(current_row);
                    }

                    let mut y = 0.0;
                    for row in &rows {
                        let mut row_height: f64 = 0.0;
                        let row_total_width: f64 = row
                            .iter()
                            .map(|&i| layout_box.children[i].dimensions.margin_box().width)
                            .sum();
                        let row_gap = gap * row.len().saturating_sub(1) as f64;
                        let row_remaining = (content_width - row_total_width - row_gap).max(0.0);

                        let (initial_x, inter_item_space) =
                            compute_justify(&justify_content, row_remaining, row.len(), gap);
                        let mut x = initial_x;

                        for (ri, &child_idx) in row.iter().enumerate() {
                            let child = &mut layout_box.children[child_idx];
                            child.dimensions.content.x = x
                                + child.dimensions.margin.left
                                + child.dimensions.border.left
                                + child.dimensions.padding.left;
                            child.dimensions.content.y = y
                                + child.dimensions.margin.top
                                + child.dimensions.border.top
                                + child.dimensions.padding.top;
                            x += child.dimensions.margin_box().width;
                            if ri < row.len().saturating_sub(1) {
                                x += inter_item_space;
                            }
                            row_height = row_height.max(child.dimensions.margin_box().height);
                        }
                        y += row_height;
                    }
                    layout_box.dimensions.content.height = y;
                } else {
                    // No wrapping — single row
                    let (initial_x, inter_item_space) =
                        compute_justify(&justify_content, remaining_space, num_children, gap);

                    let mut x = initial_x;
                    let mut max_height: f64 = 0.0;

                    for (i, child) in layout_box.children.iter_mut().enumerate() {
                        child.dimensions.content.x = x
                            + child.dimensions.margin.left
                            + child.dimensions.border.left
                            + child.dimensions.padding.left;
                        child.dimensions.content.y = child.dimensions.margin.top
                            + child.dimensions.border.top
                            + child.dimensions.padding.top;
                        x += child.dimensions.margin_box().width;
                        if i < num_children.saturating_sub(1) {
                            x += inter_item_space;
                        }
                        max_height = max_height.max(child.dimensions.margin_box().height);
                    }

                    layout_box.dimensions.content.height = max_height;

                    // === STEP 4: Apply align-items on cross axis ===
                    let align_items = layout_box
                        .style
                        .get(&CssProperty::AlignItems)
                        .and_then(|v| {
                            if let CssValue::Keyword(s) = v {
                                Some(s.as_str())
                            } else {
                                None
                            }
                        })
                        .unwrap_or("stretch");

                    let line_height = layout_box
                        .children
                        .iter()
                        .map(|c| c.dimensions.margin_box().height)
                        .fold(0.0_f64, f64::max);

                    for child in layout_box.children.iter_mut() {
                        let effective_align = child
                            .style
                            .get(&CssProperty::AlignSelf)
                            .and_then(|v| {
                                if let CssValue::Keyword(s) = v {
                                    Some(s.clone())
                                } else {
                                    None
                                }
                            })
                            .filter(|s| s != "auto" && !s.is_empty())
                            .unwrap_or_else(|| align_items.to_string());

                        let child_height = child.dimensions.margin_box().height;
                        let y_offset = match effective_align.as_str() {
                            "flex-start" | "start" | "self-start" => 0.0,
                            "flex-end" | "end" | "self-end" => line_height - child_height,
                            "center" => (line_height - child_height) / 2.0,
                            "stretch" => {
                                if child
                                    .style
                                    .get(&CssProperty::Height)
                                    .map(|v| matches!(v, CssValue::Auto))
                                    .unwrap_or(true)
                                {
                                    child.dimensions.content.height = (line_height
                                        - child.dimensions.margin.vertical()
                                        - child.dimensions.padding.vertical()
                                        - child.dimensions.border.vertical())
                                    .max(0.0);
                                }
                                0.0
                            }
                            _ => 0.0,
                        };
                        child.dimensions.content.y += y_offset;
                    }
                }
            }
        }
    }

    /// Apply min/max size constraints after layout.
    fn apply_size_constraints(
        &self,
        layout_box: &mut LayoutBox,
        container_width: f64,
        font_size: f64,
    ) {
        let w = layout_box.dimensions.content.width;
        let h = layout_box.dimensions.content.height;

        if let Some(min_w) = layout_box
            .style
            .get(&CssProperty::MinWidth)
            .and_then(|v| v.as_px(container_width, font_size, self.root_font_size))
        {
            layout_box.dimensions.content.width = w.max(min_w);
        }

        if let Some(max_w) = layout_box
            .style
            .get(&CssProperty::MaxWidth)
            .and_then(|v| v.as_px(container_width, font_size, self.root_font_size))
        {
            layout_box.dimensions.content.width = layout_box.dimensions.content.width.min(max_w);
        }

        if let Some(min_h) = layout_box
            .style
            .get(&CssProperty::MinHeight)
            .and_then(|v| v.as_px(0.0, font_size, self.root_font_size))
        {
            layout_box.dimensions.content.height = h.max(min_h);
        }

        if let Some(max_h) = layout_box
            .style
            .get(&CssProperty::MaxHeight)
            .and_then(|v| v.as_px(0.0, font_size, self.root_font_size))
        {
            layout_box.dimensions.content.height = layout_box.dimensions.content.height.min(max_h);
        }
    }

    /// Layout a replaced element (e.g., img).
    fn layout_replaced(&self, layout_box: &mut LayoutBox, container_width: f64, font_size: f64) {
        let width = layout_box
            .style
            .get(&CssProperty::Width)
            .and_then(|v| v.as_px(container_width, font_size, self.root_font_size))
            .or_else(|| {
                layout_box
                    .attributes
                    .get("width")
                    .and_then(|w| w.parse::<f64>().ok())
            })
            .unwrap_or(300.0); // Default image width

        let height = layout_box
            .style
            .get(&CssProperty::Height)
            .and_then(|v| v.as_px(0.0, font_size, self.root_font_size))
            .or_else(|| {
                layout_box
                    .attributes
                    .get("height")
                    .and_then(|h| h.parse::<f64>().ok())
            })
            .unwrap_or(150.0); // Default image height

        layout_box.dimensions.content.width = width;
        layout_box.dimensions.content.height = height;
    }

    /// Recursively find table cells and layout their children using the cell's
    /// content width as the container width.
    #[allow(clippy::only_used_in_recursion)]
    fn layout_table_cell_contents(&self, node: &mut LayoutBox, font_size: f64) {
        for child in &mut node.children {
            match child.box_type {
                LayoutBoxType::TableCell => {
                    let cell_width = child.dimensions.content.width;
                    let _cell_fs = child
                        .style
                        .font_size_px(self.root_font_size, self.root_font_size);

                    // Layout cell children with the cell's computed content width
                    let mut y = 0.0;
                    for cell_child in &mut child.children {
                        self.compute_layout(cell_child, cell_width);
                        cell_child.dimensions.content.y = y;
                        // For inline/text children, force content.width to cell
                        // width so text-align works against the full cell area.
                        if matches!(
                            cell_child.box_type,
                            LayoutBoxType::Inline | LayoutBoxType::AnonymousInline
                        ) {
                            cell_child.dimensions.content.width = cell_width;
                        }
                        y += cell_child.dimensions.margin_box().height;
                    }
                }
                _ => {
                    // Recurse through rows, thead, tbody, etc.
                    self.layout_table_cell_contents(child, font_size);
                }
            }
        }
    }

    /// Pre-compute box-model (padding, border, margin) for all rows and cells
    /// inside a table, so that `table_layout::layout_table` can use them.
    fn precompute_table_box_models(&self, table_box: &mut LayoutBox, container_width: f64) {
        self.precompute_table_children_box_models(&mut table_box.children, container_width);
    }

    fn precompute_table_children_box_models(
        &self,
        children: &mut [LayoutBox],
        container_width: f64,
    ) {
        for child in children.iter_mut() {
            match child.box_type {
                LayoutBoxType::TableRow => {
                    // Compute row box model (usually minimal)
                    let fs = child
                        .style
                        .font_size_px(self.root_font_size, self.root_font_size);
                    self.compute_box_model(child, container_width, fs);

                    // Compute cell box models
                    for cell in child.children.iter_mut() {
                        if matches!(cell.box_type, LayoutBoxType::TableCell) {
                            let cfs = cell
                                .style
                                .font_size_px(self.root_font_size, self.root_font_size);
                            self.compute_box_model(cell, container_width, cfs);
                        }
                    }
                }
                LayoutBoxType::Block | LayoutBoxType::AnonymousBlock => {
                    let tag = child.tag_name.as_deref().unwrap_or("");
                    if matches!(tag, "thead" | "tbody" | "tfoot") {
                        let fs = child
                            .style
                            .font_size_px(self.root_font_size, self.root_font_size);
                        self.compute_box_model(child, container_width, fs);
                        self.precompute_table_children_box_models(
                            &mut child.children,
                            container_width,
                        );
                    }
                }
                _ => {}
            }
        }
    }
}

/// Compute initial x-offset and inter-item spacing for flex justify-content.
fn compute_justify(
    justify: &str,
    remaining_space: f64,
    num_children: usize,
    gap: f64,
) -> (f64, f64) {
    if num_children == 0 {
        return (0.0, gap);
    }
    match justify {
        "center" => (remaining_space / 2.0, gap),
        "flex-end" | "end" => (remaining_space, gap),
        "space-between" => {
            if num_children <= 1 {
                (0.0, gap)
            } else {
                let space = remaining_space / (num_children - 1) as f64 + gap;
                (0.0, space)
            }
        }
        "space-around" => {
            let per_item = remaining_space / num_children as f64;
            (per_item / 2.0, per_item + gap)
        }
        "space-evenly" => {
            let space = remaining_space / (num_children + 1) as f64;
            (space, space + gap)
        }
        _ => (0.0, gap), // flex-start / start / default
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::css::CssParser;
    use crate::html::HtmlParser;

    #[test]
    fn test_basic_layout() {
        let html = "<html><body><h1>Hello</h1><p>World</p></body></html>";
        let dom = HtmlParser::parse(html).unwrap();
        let stylesheet = CssParser::parse("h1 { font-size: 24px; } p { margin: 10px; }").unwrap();

        let engine = LayoutEngine::new(PageLayout::default());
        let pages = engine.layout(&dom, &[stylesheet]).unwrap();

        assert!(!pages.is_empty());
    }

    #[test]
    fn test_page_break() {
        let html = r#"<html><body>
            <div>Page 1</div>
            <div style="page-break-before: always;">Page 2</div>
        </body></html>"#;
        let dom = HtmlParser::parse(html).unwrap();
        let engine = LayoutEngine::new(PageLayout::default());
        let pages = engine.layout(&dom, &[]).unwrap();

        assert!(pages.len() >= 1);
    }

    #[test]
    fn test_flex_align_items_center() {
        let html = r#"<div style="display:flex;align-items:center;height:100px">
            <span style="height:20px">A</span>
            <span style="height:40px">B</span>
        </div>"#;
        let dom = HtmlParser::parse(html).unwrap();
        let engine = LayoutEngine::new(PageLayout::default());
        let pages = engine.layout(&dom, &[]).unwrap();
        assert!(!pages.is_empty());
    }

    #[test]
    fn test_min_width_constraint() {
        let html = r#"<div style="min-width:200px;width:50px">content</div>"#;
        let dom = HtmlParser::parse(html).unwrap();
        let engine = LayoutEngine::new(PageLayout::default());
        let pages = engine.layout(&dom, &[]).unwrap();
        assert!(!pages.is_empty());
    }

    #[test]
    fn test_flex_shrink_no_overflow() {
        let html = r#"<div style="display:flex;width:200px">
            <div style="flex-shrink:1">Item 1</div>
            <div style="flex-shrink:1">Item 2</div>
            <div style="flex-shrink:1">Item 3</div>
        </div>"#;
        let dom = HtmlParser::parse(html).unwrap();
        let engine = LayoutEngine::new(PageLayout::default());
        let pages = engine.layout(&dom, &[]).unwrap();
        assert!(!pages.is_empty());
    }
}
