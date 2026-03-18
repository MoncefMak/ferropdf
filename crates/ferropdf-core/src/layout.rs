use crate::{ComputedStyle, NodeId, Rect, Insets};

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id: u16,
    pub x:        f32,
    pub y:        f32,
    pub advance:  f32,
    pub font_id:  u64,
}

#[derive(Debug, Clone)]
pub struct ShapedLine {
    pub glyphs: Vec<ShapedGlyph>,
    pub width:  f32,
    pub y:      f32,
    /// The text content of this line (for encoding in the PDF).
    pub text:   String,
}

// =============================================================================
// BreakUnit — Unité sécable pour la pagination intelligente
// =============================================================================
// Après le layout Taffy + shaping cosmic-text, on construit une liste PLATE
// d'unités sécables. Chaque unité est la plus petite entité déplaçable sans
// casser le sens du document.
// =============================================================================

/// Une unité sécable — la plus petite entité qui peut être déplacée
/// sans casser le sens du document.
#[derive(Debug, Clone)]
pub enum BreakUnit {
    /// Une ligne individuelle issue des layout_runs() de cosmic-text.
    TextLine {
        /// Coordonnée Y du haut de la ligne (espace continu absolu, en pt).
        y_top: f32,
        /// Coordonnée Y du bas de la ligne (espace continu absolu, en pt).
        y_bottom: f32,
        /// Index de la ligne dans son paragraphe parent.
        line_index: usize,
        /// NodeId du nœud texte parent (pour regrouper les lignes d'un même paragraphe).
        parent_node: Option<NodeId>,
        /// Contenu shapé de la ligne.
        content: ShapedLine,
    },
    /// Bloc non sécable (image, conteneur avec break-inside: avoid).
    Atomic {
        /// Coordonnée Y du haut du bloc (espace continu absolu, en pt).
        y_top: f32,
        /// Coordonnée Y du bas du bloc (espace continu absolu, en pt).
        y_bottom: f32,
        /// Le LayoutBox complet.
        node: LayoutBox,
    },
    /// Marqueur de saut de page forcé (break-before: page).
    ForcedBreak,
}

impl BreakUnit {
    /// Y du haut de l'unité dans l'espace continu (pt).
    pub fn y_top(&self) -> f32 {
        match self {
            BreakUnit::TextLine { y_top, .. } => *y_top,
            BreakUnit::Atomic { y_top, .. } => *y_top,
            BreakUnit::ForcedBreak => 0.0,
        }
    }

    /// Y du bas de l'unité dans l'espace continu (pt).
    pub fn y_bottom(&self) -> f32 {
        match self {
            BreakUnit::TextLine { y_bottom, .. } => *y_bottom,
            BreakUnit::Atomic { y_bottom, .. } => *y_bottom,
            BreakUnit::ForcedBreak => 0.0,
        }
    }
}

#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub node_id:      Option<NodeId>,
    pub style:        ComputedStyle,
    /// Border-box rectangle (x, y, width, height) in absolute coordinates.
    pub rect:         Rect,
    pub content:      Rect,
    pub padding:      Insets,
    pub border:       Insets,
    pub margin:       Insets,
    pub children:     Vec<LayoutBox>,
    pub shaped_lines: Vec<ShapedLine>,
    pub image_src:    Option<String>,
    pub text_content: Option<String>,
    /// True if this box is absolutely positioned (out of normal flow).
    pub out_of_flow:      bool,
    /// Visual offset from position: relative (does not affect flow).
    pub visual_offset_x:  f32,
    pub visual_offset_y:  f32,
}

impl Default for LayoutBox {
    fn default() -> Self {
        Self {
            node_id:      None,
            style:        ComputedStyle::default(),
            rect:         Rect::zero(),
            content:      Rect::zero(),
            padding:      Insets::zero(),
            border:       Insets::zero(),
            margin:       Insets::zero(),
            children:     Vec::new(),
            shaped_lines: Vec::new(),
            image_src:    None,
            text_content: None,
            out_of_flow:     false,
            visual_offset_x: 0.0,
            visual_offset_y: 0.0,
        }
    }
}

impl LayoutBox {
    pub fn border_box(&self) -> Rect {
        Rect::new(
            self.content.x - self.padding.left - self.border.left,
            self.content.y - self.padding.top  - self.border.top,
            self.content.width  + self.padding.horizontal() + self.border.horizontal(),
            self.content.height + self.padding.vertical()   + self.border.vertical(),
        )
    }

    pub fn margin_box_height(&self) -> f32 {
        self.margin.top + self.border.top + self.padding.top
        + self.content.height
        + self.padding.bottom + self.border.bottom + self.margin.bottom
    }

    pub fn is_text_leaf(&self) -> bool {
        self.text_content.is_some() && self.children.is_empty()
    }
}

#[derive(Debug, Default)]
pub struct LayoutTree {
    pub root: Option<LayoutBox>,
}

impl LayoutTree {
    pub fn new() -> Self { Self::default() }

    /// Return references to the root's direct children.
    pub fn root_children_boxes(&self) -> Vec<&LayoutBox> {
        match &self.root {
            Some(root) => root.children.iter().collect(),
            None => Vec::new(),
        }
    }
}

/// Une page paginée = un sous-ensemble du LayoutTree
#[derive(Debug, Clone)]
pub struct Page {
    pub page_number: u32,
    pub total_pages: u32,
    pub content:     Vec<LayoutBox>,
    pub margin_boxes: Vec<MarginBox>,
}

#[derive(Debug, Clone)]
pub struct MarginBox {
    pub position: MarginBoxPosition,
    pub rect:     Rect,
    pub text:     String,
    pub style:    ComputedStyle,
}

#[derive(Debug, Clone, PartialEq)]
pub enum MarginBoxPosition {
    TopLeft, TopCenter, TopRight,
    BottomLeft, BottomCenter, BottomRight,
}
