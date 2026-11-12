use ferropdf_core::*;

/// Apply tag-specific defaults (e.g. <b> is bold, <em> is italic)
pub fn apply_tag_defaults(style: &mut ComputedStyle, tag: Option<&str>) {
    match tag {
        Some(ferropdf_core::dom::PSEUDO_BEFORE | ferropdf_core::dom::PSEUDO_AFTER) => {
            style.display = Display::Inline;
        }
        Some("b") | Some("strong") => {
            style.display = Display::Inline;
            style.font_weight = FontWeight::Bold;
        }
        Some("i") | Some("em") => {
            style.display = Display::Inline;
            style.font_style = FontStyle::Italic;
        }
        Some("a") => {
            style.display = Display::Inline;
            style.text_decoration = style::TextDecoration::Underline;
            if style.color == Color::black() {
                style.color = Color::from_hex("#0000ee").unwrap_or(Color::black());
            }
        }
        Some("span") | Some("code") | Some("kbd") | Some("samp") | Some("var") | Some("abbr")
        | Some("cite") | Some("dfn") | Some("q") | Some("small") | Some("sub") | Some("sup")
        | Some("time") | Some("mark") | Some("s") | Some("u") | Some("del") | Some("ins")
        | Some("label") => {
            style.display = Display::Inline;
        }
        _ => {}
    }
}

/// Resolve relative units (em, rem, px, mm) to pt (points typographiques)
pub fn resolve_units(
    style: &mut ComputedStyle,
    _parent_style: Option<&ComputedStyle>,
    root_font_size: f32,
) {
    let font_size = style.font_size;

    // Resolve margin
    for m in &mut style.margin {
        if let Some(pt) = m.to_pt(font_size, root_font_size) {
            *m = Length::Pt(pt);
        } else {
            resolve_length_em_rem(m, font_size, root_font_size);
        }
    }

    // Resolve padding
    for p in &mut style.padding {
        if let Some(pt) = p.to_pt(font_size, root_font_size) {
            *p = Length::Pt(pt);
        } else {
            resolve_length_em_rem(p, font_size, root_font_size);
        }
    }
    for inset in [
        &mut style.top,
        &mut style.right,
        &mut style.bottom,
        &mut style.left,
    ] {
        resolve_length_em_rem(inset, font_size, root_font_size);
    }

    // Text spacing and indent: absolute units to pt (`em` uses this
    // element's font size, so children inherit the computed length).
    for l in [
        &mut style.letter_spacing,
        &mut style.word_spacing,
        &mut style.text_indent,
    ] {
        resolve_length_em_rem(l, font_size, root_font_size);
    }

    // Border radii: absolute units to pt; percentages are resolved against
    // the border box at paint time.
    for (x, y) in &mut style.border_radius.corners {
        resolve_length_em_rem(x, font_size, root_font_size);
        resolve_length_em_rem(y, font_size, root_font_size);
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

    // line-height against this element's final font size. `normal` and
    // numbers stay factors so children re-resolve them against their own
    // font size; lengths become absolute and children inherit that value.
    let normal = font_size * LineHeight::NORMAL_FACTOR;
    style.line_height = match style.line_height_spec {
        LineHeight::Normal => normal,
        LineHeight::Number(n) => n * font_size,
        LineHeight::Length(l) => {
            let pt = match l {
                Length::Percent(p) => p / 100.0 * font_size,
                Length::Calc(c) => {
                    let c = c.resolve_font_relative(font_size, root_font_size);
                    c.pt + c.percent / 100.0 * font_size
                }
                other => other.to_pt(font_size, root_font_size).unwrap_or(normal),
            }
            .max(0.0);
            style.line_height_spec = LineHeight::Length(Length::Pt(pt));
            pt
        }
    };
}

fn resolve_length_em_rem(length: &mut Length, font_size: f32, root_font_size: f32) {
    match length {
        Length::Em(v) => *length = Length::Pt(*v * font_size),
        Length::Rem(v) => *length = Length::Pt(*v * root_font_size),
        Length::Px(v) => *length = Length::Pt(*v * 0.75), // 1px = 72/96 pt
        Length::Mm(v) => *length = Length::Pt(*v * 2.834_646),
        Length::Calc(c) => {
            *length = c
                .resolve_font_relative(font_size, root_font_size)
                .simplify()
        }
        _ => {} // Pt, Percent, Auto, Zero, None — keep as is
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tag_defaults_bold() {
        let mut style = ComputedStyle::default();
        apply_tag_defaults(&mut style, Some("strong"));
        assert_eq!(style.display, Display::Inline);
        assert_eq!(style.font_weight, FontWeight::Bold);
    }

    #[test]
    fn tag_defaults_italic() {
        let mut style = ComputedStyle::default();
        apply_tag_defaults(&mut style, Some("em"));
        assert_eq!(style.display, Display::Inline);
        assert_eq!(style.font_style, FontStyle::Italic);
    }

    #[test]
    fn tag_defaults_link() {
        let mut style = ComputedStyle::default();
        apply_tag_defaults(&mut style, Some("a"));
        assert_eq!(style.display, Display::Inline);
        assert_eq!(style.text_decoration, style::TextDecoration::Underline);
    }

    #[test]
    fn tag_defaults_span_inline() {
        let mut style = ComputedStyle {
            display: Display::Block,
            ..Default::default()
        };
        apply_tag_defaults(&mut style, Some("span"));
        assert_eq!(style.display, Display::Inline);
    }

    #[test]
    fn resolve_units_em_margin() {
        let mut style = ComputedStyle {
            font_size: 16.0, // 16pt
            margin: [Length::Em(2.0); 4],
            ..Default::default()
        };
        resolve_units(&mut style, None, 12.0);
        // 2em * 16pt = 32pt
        assert_eq!(style.margin[0], Length::Pt(32.0));
    }

    #[test]
    fn resolve_units_px_padding() {
        let mut style = ComputedStyle {
            font_size: 12.0,
            padding: [Length::Px(20.0); 4],
            ..Default::default()
        };
        resolve_units(&mut style, None, 12.0);
        // 20px * 0.75 = 15pt
        assert_eq!(style.padding[0], Length::Pt(15.0));
    }

    #[test]
    fn resolve_units_preserves_percent() {
        let mut style = ComputedStyle {
            width: Length::Percent(50.0),
            ..Default::default()
        };
        resolve_units(&mut style, None, 12.0);
        assert_eq!(style.width, Length::Percent(50.0));
    }

    #[test]
    fn resolve_units_preserves_auto() {
        let mut style = ComputedStyle {
            width: Length::Auto,
            ..Default::default()
        };
        resolve_units(&mut style, None, 12.0);
        assert_eq!(style.width, Length::Auto);
    }

    fn line_height_for(spec: LineHeight, font_size: f32) -> (f32, LineHeight) {
        let mut style = ComputedStyle {
            font_size,
            line_height_spec: spec,
            ..Default::default()
        };
        resolve_units(&mut style, None, 12.0);
        (style.line_height, style.line_height_spec)
    }

    #[test]
    fn line_height_normal_follows_own_font_size() {
        // A 24pt heading inheriting `normal` from a 12pt body gets 1.2 × 24pt,
        // and a 10pt body gets 12pt rather than the 14.4pt of the root default.
        assert!((line_height_for(LineHeight::Normal, 24.0).0 - 28.8).abs() < 0.01);
        assert!((line_height_for(LineHeight::Normal, 10.0).0 - 12.0).abs() < 0.01);
    }

    #[test]
    fn line_height_number_stays_a_factor() {
        let (pt, spec) = line_height_for(LineHeight::Number(1.5), 10.0);
        assert!((pt - 15.0).abs() < 0.01);
        assert_eq!(spec, LineHeight::Number(1.5), "numbers inherit as factors");
    }

    #[test]
    fn line_height_lengths_become_absolute() {
        for (l, want) in [
            (Length::Em(1.5), 15.0),
            (Length::Percent(150.0), 15.0),
            (Length::Rem(2.0), 24.0),
            (Length::Px(20.0), 15.0),
            (Length::Pt(18.0), 18.0),
        ] {
            let (pt, spec) = line_height_for(LineHeight::Length(l), 10.0);
            assert!((pt - want).abs() < 0.01, "{l:?}: {pt}");
            assert_eq!(spec, LineHeight::Length(Length::Pt(want)), "{l:?}");
        }
    }

    #[test]
    fn resolve_length_em_rem_conversion() {
        let mut l = Length::Em(2.0);
        resolve_length_em_rem(&mut l, 16.0, 12.0);
        assert_eq!(l, Length::Pt(32.0));

        let mut l = Length::Rem(1.5);
        resolve_length_em_rem(&mut l, 16.0, 12.0);
        assert_eq!(l, Length::Pt(18.0));

        let mut l = Length::Mm(10.0);
        resolve_length_em_rem(&mut l, 16.0, 12.0);
        if let Length::Pt(v) = l {
            assert!((v - 28.346).abs() < 0.01);
        } else {
            panic!("expected Pt");
        }
    }
}
