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
        visibility: parent.visibility,
        orphans: parent.orphans,
        widows: parent.widows,
        border_collapse: parent.border_collapse.clone(),
        list_style_type: parent.list_style_type.clone(),
        ..Default::default()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inherits_color() {
        let mut parent = ComputedStyle::default();
        parent.color = Color::from_rgb8(255, 0, 0);
        let child = inherit_from(&parent);
        assert_eq!(child.color, Color::from_rgb8(255, 0, 0));
    }

    #[test]
    fn inherits_font_properties() {
        let mut parent = ComputedStyle::default();
        parent.font_size = 24.0;
        parent.font_weight = FontWeight::Bold;
        parent.font_style = FontStyle::Italic;
        parent.font_family = vec!["Georgia".to_string()];
        let child = inherit_from(&parent);
        assert_eq!(child.font_size, 24.0);
        assert_eq!(child.font_weight, FontWeight::Bold);
        assert_eq!(child.font_style, FontStyle::Italic);
        assert_eq!(child.font_family, vec!["Georgia".to_string()]);
    }

    #[test]
    fn does_not_inherit_margin() {
        let mut parent = ComputedStyle::default();
        parent.margin = [Length::Pt(20.0); 4];
        let child = inherit_from(&parent);
        assert_eq!(child.margin, [Length::Zero; 4]);
    }

    #[test]
    fn does_not_inherit_padding() {
        let mut parent = ComputedStyle::default();
        parent.padding = [Length::Pt(10.0); 4];
        let child = inherit_from(&parent);
        assert_eq!(child.padding, [Length::Zero; 4]);
    }

    #[test]
    fn does_not_inherit_background() {
        let mut parent = ComputedStyle::default();
        parent.background_color = Color::from_rgb8(255, 0, 0);
        let child = inherit_from(&parent);
        assert!(child.background_color.is_transparent());
    }

    #[test]
    fn does_not_inherit_display() {
        let mut parent = ComputedStyle::default();
        parent.display = Display::Flex;
        let child = inherit_from(&parent);
        assert_eq!(child.display, Display::Block); // default
    }

    #[test]
    fn inherits_orphans_widows() {
        let mut parent = ComputedStyle::default();
        parent.orphans = 5;
        parent.widows = 3;
        let child = inherit_from(&parent);
        assert_eq!(child.orphans, 5);
        assert_eq!(child.widows, 3);
    }

    #[test]
    fn inherits_text_align() {
        let mut parent = ComputedStyle::default();
        parent.text_align = TextAlign::Center;
        let child = inherit_from(&parent);
        assert_eq!(child.text_align, TextAlign::Center);
    }
}
