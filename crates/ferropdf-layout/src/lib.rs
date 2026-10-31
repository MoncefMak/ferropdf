pub mod style_to_taffy;
pub mod table_layout;
mod taffy_bridge;
pub mod text;

use ferropdf_core::{Display, Document, InlineSpan, LayoutBox, LayoutTree};
use ferropdf_style::StyleTree;
pub use text::{FontDatabase, FontSession};

/// Build a layout tree from a styled document using Taffy for layout computation.
/// All coordinates are in typographic points (pt).
pub fn layout(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
) -> ferropdf_core::Result<LayoutTree> {
    let owned_font_db = FontDatabase::new();
    layout_with_fonts(
        document,
        styles,
        available_width,
        available_height,
        &owned_font_db,
    )
}

/// Same as `layout` but reuses an existing FontDatabase (avoids reloading system fonts).
pub fn layout_with_fonts(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
    font_db: &FontDatabase,
) -> ferropdf_core::Result<LayoutTree> {
    let mut session = font_db.session();
    layout_with_font_system(
        document,
        styles,
        available_width,
        available_height,
        session.font_system(),
    )
}

/// Lay out one element on its own at `width` (used for running elements,
/// `position: running(name)`, drawn in page margin boxes). Returns the box
/// with its border box at the origin.
pub fn layout_element(
    document: &Document,
    styles: &StyleTree,
    node: ferropdf_core::NodeId,
    width: f32,
    font_system: &mut cosmic_text::FontSystem,
) -> ferropdf_core::Result<Option<LayoutBox>> {
    let mut tree = taffy_bridge::build_layout_from(document, styles, font_system, node, width)?;
    if let Some(ref mut root) = tree.root {
        populate_shaped_lines(root, font_system);
    }
    Ok(tree.root)
}

/// Layout with an already checked-out font system (see [`FontDatabase::session`]).
pub fn layout_with_font_system(
    document: &Document,
    styles: &StyleTree,
    available_width: f32,
    available_height: f32,
    font_system: &mut cosmic_text::FontSystem,
) -> ferropdf_core::Result<LayoutTree> {
    let mut tree = taffy_bridge::build_layout(
        document,
        styles,
        font_system,
        available_width,
        available_height,
    )?;

    // NOTE: block_flow post-pass disabled.
    // Taffy 0.5 with block_layout feature handles margin collapsing natively.
    // The post-pass was propagating pending margins through containers without
    // BFC boundaries, causing cascading Y shifts and text overlap.
    // block_flow::apply_block_flow(&mut tree, available_width);

    // Post-pass: shape text at final content widths to populate shaped_lines.
    // This is the SINGLE SOURCE OF TRUTH for line breaks — pdf.rs must not re-wrap.
    if let Some(ref mut root) = tree.root {
        populate_shaped_lines(root, font_system);
    }

    Ok(tree)
}

/// Walk the LayoutBox tree and populate `shaped_lines` for each text node
/// by re-shaping text at the box's final content width.
///
/// When a block container has all-inline children (text + `<strong>`, `<em>`, etc.),
/// their text is merged into a single rich-text buffer and shaped as one paragraph.
/// This ensures correct word-wrapping across span boundaries.
fn populate_shaped_lines(lb: &mut LayoutBox, fs: &mut cosmic_text::FontSystem) {
    populate_shaped_lines_inner(lb, fs, (true, true));
}

/// `edges` = (trim start, trim end): whether this box starts/ends a line box,
/// i.e. has no inline-level neighbour on that side. Spaces next to an inline
/// sibling (`text <b>bold</b>`, `label <span class=btn>…</span>`) are kept.
fn populate_shaped_lines_inner(
    lb: &mut LayoutBox,
    fs: &mut cosmic_text::FontSystem,
    edges: (bool, bool),
) {
    // Check if this block should merge its inline children
    if should_merge_inline_children(lb) {
        merge_inline_children(lb, fs);
        return;
    }

    if let Some(ref txt) = lb.text_content {
        if !txt.trim().is_empty() && lb.shaped_lines.is_empty() {
            let style = &lb.style;
            let font_family = style.font_family.first().map(|s| s.as_str()).unwrap_or("");
            // A lone text leaf is its own line box: collapsible spaces at
            // its edges are removed (CSS Text 3 §4.1.2).
            let txt = if collapses(style.white_space) {
                let t = if edges.0 {
                    txt.trim_start_matches(' ')
                } else {
                    txt.as_str()
                };
                if edges.1 {
                    t.trim_end_matches(' ')
                } else {
                    t
                }
            } else {
                txt.as_str()
            };
            lb.shaped_lines = text::shape_text_lines(
                txt,
                style.font_size,
                style.line_height,
                font_family,
                style.font_weight.is_bold(),
                style.font_style == ferropdf_core::FontStyle::Italic,
                lb.content.width,
                text::css_align(style.text_align.physical(style.direction)),
                style.white_space.wraps(),
                text::Spacing::of(style),
                fs,
            );
        }
    }
    let inline: Vec<bool> = lb.children.iter().map(is_inline_level).collect();
    let n = lb.children.len();
    for (i, child) in lb.children.iter_mut().enumerate() {
        let prev_inline = i > 0 && inline[i - 1];
        let next_inline = i + 1 < n && inline[i + 1];
        populate_shaped_lines_inner(child, fs, (!prev_inline, !next_inline));
    }
}

fn is_inline_level(lb: &LayoutBox) -> bool {
    if lb.is_text_leaf() {
        return lb
            .text_content
            .as_deref()
            .is_some_and(|t| !t.trim().is_empty());
    }
    matches!(lb.style.display, Display::Inline | Display::InlineBlock)
}

/// Check if a LayoutBox is a block container whose children are all inline content.
/// Returns true if inline merging should be applied.
fn should_merge_inline_children(lb: &LayoutBox) -> bool {
    // Must not be a text leaf itself
    if lb.text_content.is_some() {
        return false;
    }
    if lb.children.is_empty() {
        return false;
    }
    // Children of a flex or grid container are blockified items laid out by
    // Taffy: never merge them into one paragraph.
    if matches!(lb.style.display, Display::Flex | Display::Grid) {
        return false;
    }
    // All children must be inline-mergeable
    let parent_fs = lb.style.font_size;
    lb.children
        .iter()
        .all(|child| is_mergeable_inline(child, parent_fs))
}

/// Check if a LayoutBox can be merged as inline content.
fn is_mergeable_inline(lb: &LayoutBox, parent_fs: f32) -> bool {
    if lb.is_text_leaf() {
        // Text leaf with same font size as parent — mergeable
        return (lb.style.font_size - parent_fs).abs() < 0.1;
    }
    // Inline element whose children are all mergeable
    if lb.style.display != Display::Inline {
        return false;
    }
    lb.children
        .iter()
        .all(|child| is_mergeable_inline(child, parent_fs))
}

/// Merge inline children into a single rich-text buffer on the parent LayoutBox.
fn merge_inline_children(lb: &mut LayoutBox, fs: &mut cosmic_text::FontSystem) {
    let mut spans = Vec::new();
    // Single pass: collect spans and clear text content simultaneously
    for child in &mut lb.children {
        collect_and_clear(child, &mut spans);
    }

    if collapses(lb.style.white_space) {
        trim_collapsible_edges(&mut spans);
    }

    if spans.is_empty() || spans.iter().all(|s| s.text.trim().is_empty()) {
        // Nothing to merge — recurse normally
        for child in &mut lb.children {
            populate_shaped_lines(child, fs);
        }
        return;
    }

    // Shape merged rich text at parent's content width
    // `text-indent` indents the first line of a block container.
    let indent = if lb.style.display == Display::Inline {
        0.0
    } else {
        lb.style.text_indent.resolve_against(lb.content.width)
    };
    lb.shaped_lines = text::shape_rich_text_lines(
        &spans,
        lb.content.width,
        text::css_align(lb.style.text_align.physical(lb.style.direction)),
        lb.style.white_space.wraps(),
        indent,
        fs,
    );
    lb.inline_spans = spans;
}

/// Recursively collect InlineSpans and clear text content in a single pass.
fn collect_and_clear(lb: &mut LayoutBox, spans: &mut Vec<InlineSpan>) {
    if lb.is_text_leaf() {
        if let Some(text) = lb.text_content.take() {
            let style = &lb.style;
            spans.push(InlineSpan {
                text,
                font_size: style.font_size,
                line_height: style.line_height,
                font_family: style.font_family.first().cloned().unwrap_or_default(),
                bold: style.font_weight.is_bold(),
                italic: style.font_style == ferropdf_core::FontStyle::Italic,
                color: style.color,
                text_decoration: style.text_decoration.clone(),
                letter_spacing: style.letter_spacing.resolve_against(0.0),
                word_spacing: style.word_spacing.resolve_against(0.0),
            });
        }
    } else {
        for child in &mut lb.children {
            collect_and_clear(child, spans);
        }
    }
    lb.shaped_lines.clear();
}

fn collapses(ws: ferropdf_core::WhiteSpace) -> bool {
    !matches!(
        ws,
        ferropdf_core::WhiteSpace::Pre | ferropdf_core::WhiteSpace::PreWrap
    )
}

/// Remove collapsible spaces at the start and end of a block's inline
/// content and between adjacent spans (`"a "` + `" b"` → `"a "` + `"b"`).
/// Span texts were already collapsed per node by `process_text`.
fn trim_collapsible_edges(spans: &mut Vec<InlineSpan>) {
    let mut prev_ends_with_space = true; // block start behaves like a space
    for span in spans.iter_mut() {
        if prev_ends_with_space {
            let trimmed = span.text.trim_start_matches(' ');
            if trimmed.len() != span.text.len() {
                span.text = trimmed.to_string();
            }
        }
        if !span.text.is_empty() {
            prev_ends_with_space = span.text.ends_with(' ');
        }
    }
    if let Some(last) = spans.iter_mut().rev().find(|s| !s.text.is_empty()) {
        let trimmed = last.text.trim_end_matches(' ');
        if trimmed.len() != last.text.len() {
            last.text = trimmed.to_string();
        }
    }
    spans.retain(|s| !s.text.is_empty());
}

#[cfg(test)]
mod white_space_tests {
    use super::*;

    fn span(t: &str) -> InlineSpan {
        InlineSpan {
            text: t.to_string(),
            font_size: 12.0,
            line_height: 14.4,
            font_family: String::new(),
            bold: false,
            italic: false,
            color: ferropdf_core::Color::black(),
            text_decoration: ferropdf_core::TextDecoration::None,
            letter_spacing: 0.0,
            word_spacing: 0.0,
        }
    }

    fn texts(spans: &[InlineSpan]) -> Vec<&str> {
        spans.iter().map(|s| s.text.as_str()).collect()
    }

    #[test]
    fn trims_block_edges_and_duplicate_spaces() {
        let mut s = vec![span(" Hi "), span(" bold "), span(" there ")];
        trim_collapsible_edges(&mut s);
        assert_eq!(texts(&s), ["Hi ", "bold ", "there"]);
    }

    #[test]
    fn drops_whitespace_only_spans() {
        let mut s = vec![
            span(" "),
            span("a"),
            span(" "),
            span(" "),
            span("b"),
            span(" "),
        ];
        trim_collapsible_edges(&mut s);
        assert_eq!(texts(&s), ["a", " ", "b"]);
    }
}
