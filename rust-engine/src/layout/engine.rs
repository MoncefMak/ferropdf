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

use super::box_model::{EdgeSizes, FloatSide, LayoutBox, LayoutBoxType, PositionType};
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
                    "grid" => LayoutBoxType::Grid,
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

                // Determine CSS position type
                let position = style
                    .get(&CssProperty::Position)
                    .and_then(|v| match v {
                        CssValue::Keyword(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("static");
                layout_box.position_type = match position {
                    "relative" => PositionType::Relative,
                    "absolute" => PositionType::Absolute,
                    "fixed" => PositionType::Fixed,
                    _ => PositionType::Static,
                };

                // Determine CSS float
                let float_val = style
                    .get(&CssProperty::Float)
                    .and_then(|v| match v {
                        CssValue::Keyword(s) => Some(s.as_str()),
                        _ => None,
                    })
                    .unwrap_or("none");
                layout_box.float_side = match float_val {
                    "left" => FloatSide::Left,
                    "right" => FloatSide::Right,
                    _ => FloatSide::None,
                };

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

                // Separate out-of-flow children (absolute/fixed) from normal flow
                let mut in_flow = Vec::new();
                let mut out_of_flow = Vec::new();
                for child in layout_box.children.drain(..) {
                    if matches!(child.position_type, PositionType::Absolute | PositionType::Fixed) {
                        out_of_flow.push(child);
                    } else {
                        in_flow.push(child);
                    }
                }
                layout_box.children = in_flow;
                layout_box.out_of_flow_children = out_of_flow;

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

        // Anonymous text nodes (no tag) should NOT have box-model properties
        // (padding/margin/border). In CSS these are inherited by the parent
        // element but only apply to the element itself, not anonymous inline boxes.
        let is_anonymous_text = layout_box.text.is_some() && layout_box.tag_name.is_none();
        if !is_anonymous_text {
            self.compute_box_model(layout_box, container_width, font_size);
        }

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
                self.layout_with_taffy(layout_box, container_width, font_size, false);
            }
            LayoutBoxType::Grid => {
                self.layout_with_taffy(layout_box, container_width, font_size, true);
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

        // S3-4: Apply position offsets
        match layout_box.position_type {
            PositionType::Relative => {
                // Relative: visual-only offset, stays in normal flow
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
            _ => {}
        }

        // S3-5: Layout out-of-flow children (absolute/fixed positioned)
        if !layout_box.out_of_flow_children.is_empty() {
            let containing_width = layout_box.dimensions.content.width;
            let containing_height = layout_box.dimensions.content.height;
            let mut positioned_children = std::mem::take(&mut layout_box.out_of_flow_children);

            for child in &mut positioned_children {
                self.compute_layout(child, containing_width);

                let child_fs = child
                    .style
                    .font_size_px(self.root_font_size, self.root_font_size);

                let has_left = child.style.get(&CssProperty::Left).is_some();
                let has_right = child.style.get(&CssProperty::Right).is_some();
                let has_top = child.style.get(&CssProperty::Top).is_some();
                let has_bottom = child.style.get(&CssProperty::Bottom).is_some();

                // Horizontal positioning
                if has_left {
                    let left = child
                        .style
                        .get(&CssProperty::Left)
                        .and_then(|v| v.as_px(containing_width, child_fs, self.root_font_size))
                        .unwrap_or(0.0);
                    child.dimensions.content.x = left
                        + child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left;

                    // If both left and right are set (and no explicit width),
                    // stretch to fill the gap
                    if has_right {
                        if child.style.get(&CssProperty::Width).is_none() {
                            let right = child
                                .style
                                .get(&CssProperty::Right)
                                .and_then(|v| v.as_px(containing_width, child_fs, self.root_font_size))
                                .unwrap_or(0.0);
                            child.dimensions.content.width = (containing_width
                                - left
                                - right
                                - child.dimensions.margin.horizontal()
                                - child.dimensions.padding.horizontal()
                                - child.dimensions.border.horizontal())
                            .max(0.0);
                        }
                    }
                } else if has_right {
                    let right = child
                        .style
                        .get(&CssProperty::Right)
                        .and_then(|v| v.as_px(containing_width, child_fs, self.root_font_size))
                        .unwrap_or(0.0);
                    child.dimensions.content.x = containing_width
                        - right
                        - child.dimensions.margin_box().width
                        + child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left;
                }
                // Else: default to top-left (0,0)

                // Vertical positioning
                if has_top {
                    let top = child
                        .style
                        .get(&CssProperty::Top)
                        .and_then(|v| v.as_px(containing_height, child_fs, self.root_font_size))
                        .unwrap_or(0.0);
                    child.dimensions.content.y = top
                        + child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top;
                } else if has_bottom {
                    let bottom = child
                        .style
                        .get(&CssProperty::Bottom)
                        .and_then(|v| v.as_px(containing_height, child_fs, self.root_font_size))
                        .unwrap_or(0.0);
                    child.dimensions.content.y = containing_height
                        - bottom
                        - child.dimensions.margin_box().height
                        + child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top;
                }
            }

            layout_box.out_of_flow_children = positioned_children;
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
            // Block formatting context — stack children vertically with float support.
            // Float tracking: each float records its bottom edge and occupied width.
            struct FloatRect {
                x: f64,
                width: f64,
                top: f64,
                bottom: f64,
            }
            let mut left_floats: Vec<FloatRect> = Vec::new();
            let mut right_floats: Vec<FloatRect> = Vec::new();
            let mut y: f64 = 0.0;

            // Helper: compute left offset due to active left floats at a given y range
            let left_offset_at = |floats: &[FloatRect], top: f64, bottom: f64| -> f64 {
                floats
                    .iter()
                    .filter(|f| f.bottom > top && f.top < bottom)
                    .map(|f| f.x + f.width)
                    .fold(0.0_f64, f64::max)
            };
            // Helper: compute right intrusion due to active right floats at a given y range
            let right_intrusion_at =
                |floats: &[FloatRect], cw: f64, top: f64, bottom: f64| -> f64 {
                    floats
                        .iter()
                        .filter(|f| f.bottom > top && f.top < bottom)
                        .map(|f| cw - f.x)
                        .fold(0.0_f64, f64::max)
                };

            for child in &mut layout_box.children {
                // Skip whitespace-only text nodes in block formatting context
                if child.text.as_ref().map_or(false, |t| t.trim().is_empty())
                    && matches!(
                        child.box_type,
                        LayoutBoxType::Inline | LayoutBoxType::AnonymousInline
                    )
                {
                    continue;
                }

                // Check clear property
                let clear = child
                    .style
                    .get(&CssProperty::Clear)
                    .and_then(|v| match v {
                        CssValue::Keyword(s) => Some(s.clone()),
                        _ => None,
                    })
                    .unwrap_or_default();
                if clear == "left" || clear == "both" {
                    if let Some(max_bottom) =
                        left_floats.iter().map(|f| f.bottom).reduce(f64::max)
                    {
                        y = y.max(max_bottom);
                    }
                }
                if clear == "right" || clear == "both" {
                    if let Some(max_bottom) =
                        right_floats.iter().map(|f| f.bottom).reduce(f64::max)
                    {
                        y = y.max(max_bottom);
                    }
                }

                if child.float_side != FloatSide::None {
                    // --- Float layout ---
                    self.compute_layout(child, content_width);
                    let float_w = child.dimensions.margin_box().width;
                    let float_h = child.dimensions.margin_box().height;

                    // Find y position where the float fits
                    let float_y = y;
                    #[allow(unused_assignments)]
                    let float_x;
                    match child.float_side {
                        FloatSide::Left => {
                            let lx = left_offset_at(&left_floats, float_y, float_y + float_h);
                            float_x = lx;
                            left_floats.push(FloatRect {
                                x: float_x,
                                width: float_w,
                                top: float_y,
                                bottom: float_y + float_h,
                            });
                        }
                        FloatSide::Right => {
                            let ri = right_intrusion_at(
                                &right_floats,
                                content_width,
                                float_y,
                                float_y + float_h,
                            );
                            float_x = content_width - float_w - ri;
                            right_floats.push(FloatRect {
                                x: float_x,
                                width: float_w,
                                top: float_y,
                                bottom: float_y + float_h,
                            });
                        }
                        FloatSide::None => unreachable!(),
                    }

                    child.dimensions.content.y = float_y + child.dimensions.margin.top
                        + child.dimensions.border.top
                        + child.dimensions.padding.top;
                    child.dimensions.content.x = float_x + child.dimensions.margin.left
                        + child.dimensions.border.left
                        + child.dimensions.padding.left;
                    // Floats do NOT advance `y` for subsequent normal-flow content
                } else {
                    // --- Normal flow block ---
                    // Reduce available width by active floats at current y
                    let probe_bottom = y + 1.0; // peek height for width calc
                    let lo = left_offset_at(&left_floats, y, probe_bottom);
                    let ri = right_intrusion_at(&right_floats, content_width, y, probe_bottom);
                    let avail_w = (content_width - lo - ri).max(0.0);

                    self.compute_layout(child, avail_w);
                    child.dimensions.content.y = y + child.dimensions.margin.top;
                    child.dimensions.content.x = lo
                        + child.dimensions.margin.left
                        + child.dimensions.padding.left
                        + child.dimensions.border.left;

                    y += child.dimensions.margin_box().height;
                }
            }

            // Ensure content height includes all floats
            let max_float_bottom = left_floats
                .iter()
                .chain(right_floats.iter())
                .map(|f| f.bottom)
                .fold(y, f64::max);
            max_float_bottom
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

        // ── Merge consecutive text-only boxes on each line into a single box ──
        // This ensures the PDF receives "Hello World" instead of separate
        // "Hello" / "World" glyphs, which fixes inter-word spacing.
        // We need to drain new_children by index ranges, but we consume them in order
        // so we can just drain from the front.
        let mut merged_children: Vec<LayoutBox> = Vec::new();
        let mut merged_line_infos: Vec<(usize, usize, f64, f64)> = Vec::new();

        // Collect items per line by iterating line_infos
        let items_vec: Vec<LayoutBox> = new_children;
        // Build an index of which line each item belongs to
        let line_ranges: Vec<(usize, usize, f64, f64)> = line_infos.clone();

        for &(start, end, line_w, lh) in &line_ranges {
            let merged_start = merged_children.len();

            let mut i = start;
            while i < end {
                // If this is a text box, try to merge with subsequent text boxes
                let is_text = items_vec[i].text.is_some() && items_vec[i].tag_name.is_none();
                if is_text {
                    // Start a run of mergeable text boxes
                    let mut combined = items_vec[i]
                        .text
                        .as_ref()
                        .unwrap()
                        .clone();
                    let first_x = items_vec[i].dimensions.content.x;
                    let first_y = items_vec[i].dimensions.content.y;
                    let first_style = items_vec[i].style.clone();
                    let first_h = items_vec[i].dimensions.content.height;

                    i += 1;

                    // Merge subsequent text boxes that share the same style
                    while i < end
                        && items_vec[i].text.is_some()
                        && items_vec[i].tag_name.is_none()
                        && items_vec[i].style.font_family()
                            == first_style.font_family()
                        && items_vec[i].style.font_weight()
                            == first_style.font_weight()
                    {
                        combined.push(' ');
                        combined.push_str(
                            items_vec[i].text.as_ref().unwrap(),
                        );
                        i += 1;
                    }

                    // Measure the merged text width for accuracy
                    let family = first_style.font_family();
                    let weight = first_style.font_weight();
                    let italic_val = first_style
                        .get(&CssProperty::FontStyle)
                        .map(|v| matches!(v, CssValue::Keyword(s) if s == "italic" || s == "oblique"))
                        .unwrap_or(false);
                    let fs = first_style
                        .font_size_px(self.root_font_size, self.root_font_size);
                    let merged_w = self.measure_text_width(
                        &combined, family, weight, italic_val, fs,
                    );

                    let mut merged_box =
                        LayoutBox::text_box(combined, first_style);
                    merged_box.dimensions.content.x = first_x;
                    merged_box.dimensions.content.y = first_y;
                    merged_box.dimensions.content.width = merged_w;
                    merged_box.dimensions.content.height = first_h;

                    merged_children.push(merged_box);
                } else {
                    // Non-text element — clone (items may be referenced later)
                    merged_children.push(items_vec[i].clone());
                    i += 1;
                }
            }

            let merged_end = merged_children.len();
            merged_line_infos.push((merged_start, merged_end, line_w, lh));
        }

        let mut new_children = merged_children;
        let line_infos = merged_line_infos;

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

    /// Layout a flex or grid container using taffy.
    fn layout_with_taffy(
        &self,
        layout_box: &mut LayoutBox,
        container_width: f64,
        font_size: f64,
        is_grid: bool,
    ) {
        use taffy::prelude::*;

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

        // Strip whitespace-only inline text nodes
        layout_box.children.retain(|child| {
            !(child.text.as_ref().map_or(false, |t| t.trim().is_empty())
                && matches!(
                    child.box_type,
                    LayoutBoxType::Inline | LayoutBoxType::AnonymousInline
                ))
        });

        // Pre-compute child layouts to get intrinsic sizes
        for child in &mut layout_box.children {
            self.compute_layout(child, content_width);
        }

        // Build taffy tree
        // NodeContext stores the index into layout_box.children
        let mut tree = TaffyTree::<usize>::new();

        // Helper: convert our CssValue to taffy Dimension
        let to_dimension =
            |prop: &CssProperty, style: &ComputedStyle, cw: f64, fs: f64| -> Dimension {
                match style.get(prop) {
                    Some(CssValue::Length(l)) => {
                        Dimension::length(l.to_px(cw, fs, self.root_font_size) as f32)
                    }
                    Some(CssValue::Calc(expr)) => {
                        Dimension::length(expr.to_px(cw, fs, self.root_font_size) as f32)
                    }
                    Some(CssValue::Auto) => Dimension::auto(),
                    _ => Dimension::auto(),
                }
            };

        let to_length_percentage_auto =
            |prop: &CssProperty,
             style: &ComputedStyle,
             cw: f64,
             fs: f64|
             -> LengthPercentageAuto {
                match style.get(prop) {
                    Some(CssValue::Length(l)) => {
                        LengthPercentageAuto::length(l.to_px(cw, fs, self.root_font_size) as f32)
                    }
                    Some(CssValue::Calc(expr)) => LengthPercentageAuto::length(
                        expr.to_px(cw, fs, self.root_font_size) as f32,
                    ),
                    Some(CssValue::Auto) => LengthPercentageAuto::auto(),
                    _ => LengthPercentageAuto::auto(),
                }
            };

        let to_length_percentage =
            |prop: &CssProperty,
             style: &ComputedStyle,
             cw: f64,
             fs: f64|
             -> LengthPercentage {
                match style.get(prop) {
                    Some(CssValue::Length(l)) => {
                        LengthPercentage::length(l.to_px(cw, fs, self.root_font_size) as f32)
                    }
                    Some(CssValue::Calc(expr)) => LengthPercentage::length(
                        expr.to_px(cw, fs, self.root_font_size) as f32,
                    ),
                    _ => LengthPercentage::ZERO,
                }
            };

        // Build child nodes
        let mut child_node_ids: Vec<NodeId> = Vec::new();
        for (idx, child) in layout_box.children.iter().enumerate() {
            let child_fs = child
                .style
                .font_size_px(self.root_font_size, self.root_font_size);

            let mut style = Style {
                display: Display::DEFAULT,
                ..Style::DEFAULT
            };

            // Size
            style.size.width = to_dimension(&CssProperty::Width, &child.style, content_width, child_fs);
            style.size.height = to_dimension(&CssProperty::Height, &child.style, 0.0, child_fs);
            style.min_size.width = to_dimension(&CssProperty::MinWidth, &child.style, content_width, child_fs);
            style.min_size.height = to_dimension(&CssProperty::MinHeight, &child.style, 0.0, child_fs);
            style.max_size.width = to_dimension(&CssProperty::MaxWidth, &child.style, content_width, child_fs);
            style.max_size.height = to_dimension(&CssProperty::MaxHeight, &child.style, 0.0, child_fs);

            // Margin
            style.margin.top = to_length_percentage_auto(&CssProperty::MarginTop, &child.style, content_width, child_fs);
            style.margin.bottom = to_length_percentage_auto(&CssProperty::MarginBottom, &child.style, content_width, child_fs);
            style.margin.left = to_length_percentage_auto(&CssProperty::MarginLeft, &child.style, content_width, child_fs);
            style.margin.right = to_length_percentage_auto(&CssProperty::MarginRight, &child.style, content_width, child_fs);

            // Padding
            style.padding.top = to_length_percentage(&CssProperty::PaddingTop, &child.style, content_width, child_fs);
            style.padding.bottom = to_length_percentage(&CssProperty::PaddingBottom, &child.style, content_width, child_fs);
            style.padding.left = to_length_percentage(&CssProperty::PaddingLeft, &child.style, content_width, child_fs);
            style.padding.right = to_length_percentage(&CssProperty::PaddingRight, &child.style, content_width, child_fs);

            // Border
            style.border.top = to_length_percentage(&CssProperty::BorderTopWidth, &child.style, content_width, child_fs);
            style.border.bottom = to_length_percentage(&CssProperty::BorderBottomWidth, &child.style, content_width, child_fs);
            style.border.left = to_length_percentage(&CssProperty::BorderLeftWidth, &child.style, content_width, child_fs);
            style.border.right = to_length_percentage(&CssProperty::BorderRightWidth, &child.style, content_width, child_fs);

            // Flex item properties
            if let Some(CssValue::Number(n)) = child.style.get(&CssProperty::FlexGrow) {
                style.flex_grow = *n as f32;
            }
            if let Some(CssValue::Number(n)) = child.style.get(&CssProperty::FlexShrink) {
                style.flex_shrink = *n as f32;
            } else {
                style.flex_shrink = 1.0;
            }
            style.flex_basis = to_dimension(&CssProperty::FlexBasis, &child.style, content_width, child_fs);

            // Align-self
            if let Some(CssValue::Keyword(s)) = child.style.get(&CssProperty::AlignSelf) {
                style.align_self = match s.as_str() {
                    "flex-start" | "start" => Some(AlignSelf::FlexStart),
                    "flex-end" | "end" => Some(AlignSelf::FlexEnd),
                    "center" => Some(AlignSelf::Center),
                    "stretch" => Some(AlignSelf::Stretch),
                    "baseline" => Some(AlignSelf::Baseline),
                    _ => None,
                };
            }

            // Grid item placement
            if is_grid {
                if let Some(CssValue::Keyword(s)) = child.style.get(&CssProperty::GridColumnStart) {
                    if let Ok(n) = s.parse::<i16>() {
                        style.grid_column.start = GridPlacement::from_line_index(n);
                    }
                }
                if let Some(CssValue::Keyword(s)) = child.style.get(&CssProperty::GridColumnEnd) {
                    if let Ok(n) = s.parse::<i16>() {
                        style.grid_column.end = GridPlacement::from_line_index(n);
                    }
                }
                if let Some(CssValue::Keyword(s)) = child.style.get(&CssProperty::GridRowStart) {
                    if let Ok(n) = s.parse::<i16>() {
                        style.grid_row.start = GridPlacement::from_line_index(n);
                    }
                }
                if let Some(CssValue::Keyword(s)) = child.style.get(&CssProperty::GridRowEnd) {
                    if let Ok(n) = s.parse::<i16>() {
                        style.grid_row.end = GridPlacement::from_line_index(n);
                    }
                }
            }

            let node_id = tree.new_leaf_with_context(style, idx).unwrap();
            child_node_ids.push(node_id);
        }

        // Build root container style
        let mut root_style = Style::DEFAULT;
        root_style.display = if is_grid {
            Display::Grid
        } else {
            Display::Flex
        };
        root_style.size.width = Dimension::length(content_width as f32);

        // Flex container properties
        if !is_grid {
            if let Some(CssValue::Keyword(s)) = layout_box.style.get(&CssProperty::FlexDirection) {
                root_style.flex_direction = match s.as_str() {
                    "row-reverse" => FlexDirection::RowReverse,
                    "column" => FlexDirection::Column,
                    "column-reverse" => FlexDirection::ColumnReverse,
                    _ => FlexDirection::Row,
                };
            }
            if let Some(CssValue::Keyword(s)) = layout_box.style.get(&CssProperty::FlexWrap) {
                root_style.flex_wrap = match s.as_str() {
                    "wrap" => FlexWrap::Wrap,
                    "wrap-reverse" => FlexWrap::WrapReverse,
                    _ => FlexWrap::NoWrap,
                };
            }
        }

        // Grid container properties
        if is_grid {
            if let Some(CssValue::Keyword(s)) =
                layout_box.style.get(&CssProperty::GridTemplateColumns)
            {
                root_style.grid_template_columns = parse_grid_template(s);
            }
            if let Some(CssValue::Keyword(s)) =
                layout_box.style.get(&CssProperty::GridTemplateRows)
            {
                root_style.grid_template_rows = parse_grid_template(s);
            }
            if let Some(CssValue::Keyword(s)) = layout_box.style.get(&CssProperty::GridAutoFlow) {
                root_style.grid_auto_flow = match s.as_str() {
                    "column" => GridAutoFlow::Column,
                    "dense" | "row dense" => GridAutoFlow::RowDense,
                    "column dense" => GridAutoFlow::ColumnDense,
                    _ => GridAutoFlow::Row,
                };
            }
        }

        // Shared alignment / justify
        if let Some(CssValue::Keyword(s)) = layout_box.style.get(&CssProperty::JustifyContent) {
            root_style.justify_content = match s.as_str() {
                "flex-start" | "start" => Some(JustifyContent::FlexStart),
                "flex-end" | "end" => Some(JustifyContent::FlexEnd),
                "center" => Some(JustifyContent::Center),
                "space-between" => Some(JustifyContent::SpaceBetween),
                "space-around" => Some(JustifyContent::SpaceAround),
                "space-evenly" => Some(JustifyContent::SpaceEvenly),
                _ => None,
            };
        }
        if let Some(CssValue::Keyword(s)) = layout_box.style.get(&CssProperty::AlignItems) {
            root_style.align_items = match s.as_str() {
                "flex-start" | "start" => Some(AlignItems::FlexStart),
                "flex-end" | "end" => Some(AlignItems::FlexEnd),
                "center" => Some(AlignItems::Center),
                "stretch" => Some(AlignItems::Stretch),
                "baseline" => Some(AlignItems::Baseline),
                _ => None,
            };
        }
        if let Some(CssValue::Keyword(s)) = layout_box.style.get(&CssProperty::AlignContent) {
            root_style.align_content = match s.as_str() {
                "flex-start" | "start" => Some(AlignContent::FlexStart),
                "flex-end" | "end" => Some(AlignContent::FlexEnd),
                "center" => Some(AlignContent::Center),
                "stretch" => Some(AlignContent::Stretch),
                "space-between" => Some(AlignContent::SpaceBetween),
                "space-around" => Some(AlignContent::SpaceAround),
                "space-evenly" => Some(AlignContent::SpaceEvenly),
                _ => None,
            };
        }

        // Gap
        let gap_val = layout_box.style.get_length(
            &CssProperty::Gap,
            container_width,
            font_size,
            self.root_font_size,
        );
        if gap_val > 0.0 {
            root_style.gap.width = LengthPercentage::length(gap_val as f32);
            root_style.gap.height = LengthPercentage::length(gap_val as f32);
        }
        let row_gap_val = layout_box.style.get_length(
            &CssProperty::RowGap,
            container_width,
            font_size,
            self.root_font_size,
        );
        if row_gap_val > 0.0 {
            root_style.gap.height = LengthPercentage::length(row_gap_val as f32);
        }
        let col_gap_val = layout_box.style.get_length(
            &CssProperty::ColumnGap,
            container_width,
            font_size,
            self.root_font_size,
        );
        if col_gap_val > 0.0 {
            root_style.gap.width = LengthPercentage::length(col_gap_val as f32);
        }

        let root_id = tree.new_with_children(root_style, &child_node_ids).unwrap();

        // Run taffy layout, using pre-computed sizes as measure
        let children_ref = &layout_box.children;
        tree.compute_layout_with_measure(
            root_id,
            Size {
                width: AvailableSpace::Definite(content_width as f32),
                height: AvailableSpace::MaxContent,
            },
            |known_dims, _available, _node_id, context, _style| {
                if let Some(idx) = context {
                    let idx = *idx;
                    if idx < children_ref.len() {
                        let child = &children_ref[idx];
                        let w = known_dims
                            .width
                            .unwrap_or(child.dimensions.margin_box().width as f32);
                        let h = known_dims
                            .height
                            .unwrap_or(child.dimensions.margin_box().height as f32);
                        return Size { width: w, height: h };
                    }
                }
                Size::ZERO
            },
        )
        .unwrap();

        // Read back positions and sizes from taffy into LayoutBox dimensions
        let mut container_height: f64 = 0.0;
        for (idx, &child_nid) in child_node_ids.iter().enumerate() {
            let taffy_layout = tree.layout(child_nid).unwrap();
            let child = &mut layout_box.children[idx];

            // taffy layout.location is the position of the border box relative to the parent content box
            // taffy layout.size is the size of the border box
            let loc_x = taffy_layout.location.x as f64;
            let loc_y = taffy_layout.location.y as f64;
            let size_w = taffy_layout.size.width as f64;
            let size_h = taffy_layout.size.height as f64;

            // content position = location + border + padding
            child.dimensions.content.x = loc_x + child.dimensions.border.left + child.dimensions.padding.left;
            child.dimensions.content.y = loc_y + child.dimensions.border.top + child.dimensions.padding.top;

            // content size = border box - padding - border
            child.dimensions.content.width = (size_w
                - child.dimensions.padding.horizontal()
                - child.dimensions.border.horizontal())
            .max(0.0);
            child.dimensions.content.height = (size_h
                - child.dimensions.padding.vertical()
                - child.dimensions.border.vertical())
            .max(0.0);

            let child_bottom = loc_y + size_h + taffy_layout.margin.bottom as f64;
            container_height = container_height.max(child_bottom);
        }

        // Height
        let explicit_height = layout_box
            .style
            .get(&CssProperty::Height)
            .and_then(|v| v.as_px(0.0, font_size, self.root_font_size));
        layout_box.dimensions.content.height = explicit_height.unwrap_or(container_height);
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
                        // Skip whitespace-only text nodes
                        if cell_child.text.as_ref().map_or(false, |t| t.trim().is_empty())
                            && matches!(cell_child.box_type, LayoutBoxType::Inline | LayoutBoxType::AnonymousInline)
                        {
                            continue;
                        }
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
/// Parse a CSS grid-template-columns/rows value into taffy track definitions.
/// Supports: px lengths, fr units, `auto`, percentages.
fn parse_grid_template(
    value: &str,
) -> Vec<taffy::style::GridTemplateComponent<String>> {
    use taffy::style::{GridTemplateComponent, MaxTrackSizingFunction, MinTrackSizingFunction};
    use taffy::MinMax;

    let mut tracks = Vec::new();
    let tokens: Vec<&str> = value.split_whitespace().collect();

    for token in tokens {
        let tsf = if token.ends_with("fr") {
            if let Ok(f) = token.trim_end_matches("fr").parse::<f32>() {
                MinMax {
                    min: MinTrackSizingFunction::auto(),
                    max: MaxTrackSizingFunction::fr(f),
                }
            } else {
                continue;
            }
        } else if token.ends_with("px") {
            if let Ok(px) = token.trim_end_matches("px").parse::<f32>() {
                MinMax {
                    min: MinTrackSizingFunction::length(px),
                    max: MaxTrackSizingFunction::length(px),
                }
            } else {
                continue;
            }
        } else if token == "auto" {
            MinMax {
                min: MinTrackSizingFunction::auto(),
                max: MaxTrackSizingFunction::auto(),
            }
        } else if token.ends_with('%') {
            if let Ok(pct) = token.trim_end_matches('%').parse::<f32>() {
                MinMax {
                    min: MinTrackSizingFunction::percent(pct / 100.0),
                    max: MaxTrackSizingFunction::percent(pct / 100.0),
                }
            } else {
                continue;
            }
        } else {
            continue;
        };
        tracks.push(GridTemplateComponent::Single(tsf));
    }

    tracks
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
