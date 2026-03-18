/// Longueur CSS avant résolution.
/// Les valeurs em/rem sont résolues par ferropdf-style.
/// Les valeurs Percent sont passées à Taffy qui les résout pendant le layout.
///
/// UNITÉ INTERNE : toutes les valeurs résolues sont en POINTS TYPOGRAPHIQUES (pt).
/// 1 pt = 1/72 pouce. Les conversions depuis px/mm/cm/in sont faites lors de
/// la résolution des styles, AVANT la construction de l'arbre Taffy.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Length {
    Px(f32),
    Pt(f32),
    Mm(f32),
    Em(f32),
    Rem(f32),
    Percent(f32),
    Auto,
    Zero,
    None,
}

impl Default for Length {
    fn default() -> Self { Length::Auto }
}

impl Length {
    pub fn is_auto(&self)    -> bool { matches!(self, Length::Auto) }
    pub fn is_percent(&self) -> bool { matches!(self, Length::Percent(_)) }
    pub fn is_none(&self)    -> bool { matches!(self, Length::None) }

    /// Résoudre en points typographiques (pt) quand le contexte est connu.
    /// Retourne None pour Auto, None, Percent (résolu par Taffy).
    ///
    /// Facteurs de conversion :
    ///   1 px = 72/96 pt = 0.75 pt
    ///   1 mm = 2.834646 pt
    ///   1 em = font_size_pt
    ///   1 rem = root_font_size_pt
    pub fn to_pt(&self, font_size_pt: f32, root_font_size_pt: f32) -> Option<f32> {
        match self {
            Length::Px(v)      => Some(v * 0.75),        // 1px = 72/96 pt
            Length::Pt(v)      => Some(*v),              // identité
            Length::Mm(v)      => Some(v * 2.834_646),   // 1mm = 2.834646 pt
            Length::Em(v)      => Some(v * font_size_pt),
            Length::Rem(v)     => Some(v * root_font_size_pt),
            Length::Zero       => Some(0.0),
            Length::Percent(_) => None,
            Length::Auto       => None,
            Length::None       => None,
        }
    }
}
