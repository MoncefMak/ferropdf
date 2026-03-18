use ferropdf_core::*;

/// Apply tag-specific defaults (e.g. <b> is bold, <em> is italic)
pub fn apply_tag_defaults(style: &mut ComputedStyle, tag: Option<&str>) {
    match tag {
        Some("b") | Some("strong") => {
            style.font_weight = FontWeight::Bold;
        }
        Some("i") | Some("em") => {
            style.font_style = FontStyle::Italic;
        }
        Some("a") => {
            style.text_decoration = style::TextDecoration::Underline;
            if style.color == Color::black() {
                style.color = Color::from_hex("#0000ee").unwrap_or(Color::black());
            }
        }
        _ => {}
    }
}

/// Resolve relative units (em, rem) to px
pub fn resolve_units(
    style: &mut ComputedStyle,
    _parent_style: Option<&ComputedStyle>,
    root_font_size: f32,
) {
    let font_size = style.font_size;

    // Resolve margin
    for m in &mut style.margin {
        if let Some(px) = m.to_px(font_size, root_font_size) {
            *m = Length::Px(px);
        }
    }

    // Resolve padding
    for p in &mut style.padding {
        if let Some(px) = p.to_px(font_size, root_font_size) {
            *p = Length::Px(px);
        }
    }

    // Resolve dimensions (only em/rem, keep percent/auto for Taffy)
    resolve_length_em_rem(&mut style.width, font_size, root_font_size);
    resolve_length_em_rem(&mut style.height, font_size, root_font_size);
    resolve_length_em_rem(&mut style.min_width, font_size, root_font_size);
    resolve_length_em_rem(&mut style.max_width, font_size, root_font_size);
    resolve_length_em_rem(&mut style.min_height, font_size, root_font_size);
    resolve_length_em_rem(&mut style.max_height, font_size, root_font_size);
    resolve_length_em_rem(&mut style.flex_basis, font_size, root_font_size);
    resolve_length_em_rem(&mut style.column_gap, font_size, root_font_size);
    resolve_length_em_rem(&mut style.row_gap, font_size, root_font_size);

    // Ensure line-height is reasonable for the current font-size.
    // CSS 'normal' line-height is ~1.2 × font-size. When line_height is inherited
    // as an absolute value from a parent with a smaller font-size, it can be too
    // small (e.g. body: 16px→19.2, h2: 24px but line_height still 19.2).
    // Re-compute only if line_height < font_size (clearly inherited and stale).
    if style.line_height < font_size {
        style.line_height = font_size * 1.2;
    }
}

fn resolve_length_em_rem(length: &mut Length, font_size: f32, root_font_size: f32) {
    match length {
        Length::Em(v) => *length = Length::Px(*v * font_size),
        Length::Rem(v) => *length = Length::Px(*v * root_font_size),
        Length::Pt(v) => *length = Length::Px(*v * 1.333_333),
        Length::Mm(v) => *length = Length::Px(*v * 3.779_528),
        _ => {} // Px, Percent, Auto, Zero, None — keep as is
    }
}
