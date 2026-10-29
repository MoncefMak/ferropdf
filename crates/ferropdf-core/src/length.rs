/// Longueur CSS avant résolution.
/// Les valeurs em/rem sont résolues par ferropdf-style.
/// Les valeurs Percent sont passées à Taffy qui les résout pendant le layout.
///
/// UNITÉ INTERNE : toutes les valeurs résolues sont en POINTS TYPOGRAPHIQUES (pt).
/// 1 pt = 1/72 pouce. Les conversions depuis px/mm/cm/in sont faites lors de
/// la résolution des styles, AVANT la construction de l'arbre Taffy.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub enum Length {
    Px(f32),
    Pt(f32),
    Mm(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    #[default]
    Auto,
    Zero,
    None,
    /// `calc()` reduced to a linear combination (see [`CalcLength`]).
    Calc(CalcLength),
}

/// A `calc()` value as `percent% + pt + em·font-size + rem·root-font-size`.
/// Every valid `calc()` of lengths and percentages reduces to this form.
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct CalcLength {
    pub percent: f32,
    pub pt: f32,
    pub em: f32,
    pub rem: f32,
}

impl CalcLength {
    /// The simplest `Length` for this value (`Pt`, `Percent` or `Calc`).
    pub fn simplify(self) -> Length {
        let has_rel = self.em != 0.0 || self.rem != 0.0;
        match (self.percent != 0.0, self.pt != 0.0 || has_rel) {
            (false, false) => Length::Zero,
            (false, true) if !has_rel => Length::Pt(self.pt),
            (true, false) => Length::Percent(self.percent),
            _ => Length::Calc(self),
        }
    }

    /// Resolve `em` / `rem` into `pt`.
    pub fn resolve_font_relative(self, font_size_pt: f32, root_font_size_pt: f32) -> Self {
        Self {
            percent: self.percent,
            pt: self.pt + self.em * font_size_pt + self.rem * root_font_size_pt,
            em: 0.0,
            rem: 0.0,
        }
    }

    /// Value in pt against a percentage basis (in pt).
    pub fn against(self, basis_pt: f32) -> f32 {
        self.percent / 100.0 * basis_pt + self.pt
    }
}

impl Length {
    pub fn is_auto(&self) -> bool {
        matches!(self, Length::Auto)
    }
    pub fn is_percent(&self) -> bool {
        matches!(self, Length::Percent(_))
    }
    pub fn is_none(&self) -> bool {
        matches!(self, Length::None)
    }

    /// Resolve to typographic points (pt) when context is known.
    /// Returns None for Auto, None, Percent (resolved by Taffy).
    ///
    /// Facteurs de conversion :
    ///   1 px = 72/96 pt = 0.75 pt
    ///   1 mm = 2.834646 pt
    ///   1 em = font_size_pt
    ///   1 rem = root_font_size_pt
    pub fn to_pt(&self, font_size_pt: f32, root_font_size_pt: f32) -> Option<f32> {
        match self {
            Length::Px(v) => Some(v * 0.75),      // 1px = 72/96 pt
            Length::Pt(v) => Some(*v),            // identité
            Length::Mm(v) => Some(v * 2.834_646), // 1mm = 2.834646 pt
            Length::Em(v) => Some(v * font_size_pt),
            Length::Rem(v) => Some(v * root_font_size_pt),
            Length::Zero => Some(0.0),
            Length::Percent(_) => None,
            Length::Auto => None,
            Length::None => None,
            Length::Calc(c) if c.percent == 0.0 => {
                Some(c.resolve_font_relative(font_size_pt, root_font_size_pt).pt)
            }
            Length::Calc(_) => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn px_to_pt() {
        let result = Length::Px(16.0).to_pt(12.0, 12.0).unwrap();
        assert!((result - 12.0).abs() < 0.01, "16px = 12pt, got {}", result);
    }

    #[test]
    fn pt_identity() {
        let result = Length::Pt(24.0).to_pt(12.0, 12.0).unwrap();
        assert!((result - 24.0).abs() < 0.01);
    }

    #[test]
    fn mm_to_pt() {
        let result = Length::Mm(25.4).to_pt(12.0, 12.0).unwrap();
        // 25.4mm = 1 inch = 72pt
        assert!((result - 72.0).abs() < 0.1, "25.4mm = 72pt, got {}", result);
    }

    #[test]
    fn em_resolves_to_font_size() {
        let result = Length::Em(2.0).to_pt(16.0, 12.0).unwrap();
        assert!(
            (result - 32.0).abs() < 0.01,
            "2em * 16pt = 32pt, got {}",
            result
        );
    }

    #[test]
    fn rem_resolves_to_root_font_size() {
        let result = Length::Rem(1.5).to_pt(24.0, 12.0).unwrap();
        assert!(
            (result - 18.0).abs() < 0.01,
            "1.5rem * 12pt = 18pt, got {}",
            result
        );
    }

    #[test]
    fn auto_returns_none() {
        assert!(Length::Auto.to_pt(12.0, 12.0).is_none());
    }

    #[test]
    fn percent_returns_none() {
        assert!(Length::Percent(50.0).to_pt(12.0, 12.0).is_none());
    }

    #[test]
    fn zero_returns_zero() {
        assert_eq!(Length::Zero.to_pt(12.0, 12.0), Some(0.0));
    }
}
