use ferropdf_core::*;

/// Create a new style by inheriting inheritable properties from parent.
pub fn inherit_from(parent: &ComputedStyle) -> ComputedStyle {
    ComputedStyle {
        direction: parent.direction,
        color: parent.color,
        font_family: parent.font_family.clone(),
        font_size: parent.font_size,
        font_weight: parent.font_weight.clone(),
        font_style: parent.font_style.clone(),
        line_height: parent.line_height,
        text_align: parent.text_align,
        text_decoration: parent.text_decoration.clone(),
        letter_spacing: parent.letter_spacing,
        word_spacing: parent.word_spacing,
        text_indent: parent.text_indent,
        text_transform: parent.text_transform,
        white_space: parent.white_space,
        visibility: parent.visibility,
        orphans: parent.orphans,
        widows: parent.widows,
        border_collapse: parent.border_collapse.clone(),
        list_style_type: parent.list_style_type.clone(),
        // CSS custom properties inherit unconditionally per the
        // CSS Custom Properties spec (§2.1).
        custom_properties: parent.custom_properties.clone(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherits_color() {
        let parent = ComputedStyle {
            color: Color::from_rgb8(255, 0, 0),
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.color, Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn inherits_font_properties() {
        let parent = ComputedStyle {
            font_size: 24.0,
            font_weight: FontWeight::Bold,
            font_style: FontStyle::Italic,
            font_family: vec!["Georgia".to_string()],
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.font_size, 24.0);
        assert_eq!(child.font_weight, FontWeight::Bold);
        assert_eq!(child.font_style, FontStyle::Italic);
        assert_eq!(child.font_family, vec!["Georgia".to_string()]);
    }

    #[test]
    fn does_not_inherit_margin() {
        let parent = ComputedStyle {
            margin: [Length::Pt(20.0); 4],
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.margin, [Length::Zero; 4]);
    }

    #[test]
    fn does_not_inherit_padding() {
        let parent = ComputedStyle {
            padding: [Length::Pt(10.0); 4],
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.padding, [Length::Zero; 4]);
    }

    #[test]
    fn does_not_inherit_background() {
        let parent = ComputedStyle {
            background_color: Color::from_rgb8(255, 0, 0),
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert!(child.background_color.is_transparent());
    }

    #[test]
    fn does_not_inherit_display() {
        let parent = ComputedStyle {
            display: Display::Flex,
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.display, Display::Block); // default
    }

    #[test]
    fn inherits_orphans_widows() {
        let parent = ComputedStyle {
            orphans: 5,
            widows: 3,
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.orphans, 5);
        assert_eq!(child.widows, 3);
    }

    #[test]
    fn inherits_text_align() {
        let parent = ComputedStyle {
            text_align: TextAlign::Center,
            ..Default::default()
        };
        let child = inherit_from(&parent);
        assert_eq!(child.text_align, TextAlign::Center);
    }
}
