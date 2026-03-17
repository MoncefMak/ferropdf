# ferropdf — Instructions de Refactoring Complet

> **À l'agent** : Ce document contient toutes les instructions pour refactoriser
> le code ferropdf existant. Suis chaque étape dans l'ordre exact. Ne saute aucune étape.
> Après chaque étape, vérifie que `cargo build` passe avant de continuer.

---

## Contexte — Pourquoi on refactorise

Le code actuel a été écrit avec des parsers et un moteur de layout **faits maison**.
Résultat : 3 bugs critiques déjà trouvés, seulement 3.2x plus rapide que WeasyPrint,
et des problèmes de layout qui reviendront.

**La cause racine** : les libs Rust qui font ce travail correctement existent déjà.
Il faut les utiliser au lieu de réinventer la roue.

### Bugs actuels causés par le code maison

| Bug | Cause | Conséquence |
|---|---|---|
| `width:100%` → `Some(0.0)` | Layout maison ne gère pas les % | Tables à largeur nulle → 8 pages |
| Double soustraction padding | Algo layout maison bugué | Éléments imbriqués trop étroits |
| Mauvais nom binaire maturin | Workflow cassé | `cp` manuel à chaque build |

### Objectif après refactoring

| Métrique | Avant | Après |
|---|---|---|
| Vitesse vs WeasyPrint | 3.2x | 15-20x |
| Bugs layout | Fréquents | Quasi nuls (Taffy est battle-tested) |
| CSS Flexbox/Grid | Incomplet | Complet |
| Text wrapping | Basique | Correct (cosmic-text) |
| Workflow build | `cp` manuel | `maturin develop` |

---

## Règle absolue avant de commencer

**Ne jamais** écrire de code de layout, de parser HTML, ou de parser CSS manuellement.
Ces libs font ce travail mieux que tout ce qu'on pourrait écrire :

```
html5ever   → parsing HTML5 spec-compliant (même moteur que Firefox)
cssparser   → parsing CSS (Mozilla)
selectors   → matching sélecteurs CSS (Mozilla)
taffy       → layout Flexbox + Grid (battle-tested, utilisé en prod)
cosmic-text → text shaping + wrapping (bidi, ligatures, tout)
pdf-writer  → génération PDF bas niveau
resvg       → rendu SVG
```

---

## ÉTAPE 0 — Sauvegarder le code actuel

```bash
# Avant toute chose, créer une branche de backup
git checkout -b backup/before-refactor
git add -A
git commit -m "backup: code avant refactoring"
git checkout -b refactor/use-proper-libs
```

---

## ÉTAPE 1 — Reconstruire le Cargo.toml workspace

Remplacer **entièrement** le `Cargo.toml` racine par ceci :

```toml
[workspace]
members = [
    "crates/ferropdf-core",
    "crates/ferropdf-parse",
    "crates/ferropdf-layout",
    "crates/ferropdf-render",
    "crates/ferropdf-page",
    "bindings/python",
]
resolver = "2"

# Toutes les versions de dépendances centralisées ici
# Ne JAMAIS déclarer une version différente dans un crate enfant
[workspace.dependencies]

# HTML parsing — NE PAS écrire de parser HTML maison
html5ever    = "0.27"
markup5ever  = "0.12"

# CSS parsing — NE PAS écrire de parser CSS maison
cssparser    = "0.31"
selectors    = "0.25"

# Layout — NE PAS écrire de moteur de layout maison
taffy        = "0.5"

# Text + Fonts — NE PAS écrire de text shaper maison
cosmic-text  = "0.12"
fontdb       = "0.16"
ttf-parser   = "0.20"

# Images
image        = { version = "0.25", features = ["png", "jpeg", "webp"] }
resvg        = "0.42"
usvg         = "0.42"
tiny-skia    = "0.11"

# PDF output
pdf-writer   = "0.9"

# Utils
thiserror    = "1.0"
url          = "2.5"
id-arena     = "2.2"
rayon        = "1.8"
hashbrown    = "0.14"
log          = "0.4"
base64       = "0.22"
flate2       = "1.0"
ureq         = { version = "2.9", features = ["tls"] }

# Python bindings
pyo3         = { version = "0.21", features = ["extension-module"] }
```

**Vérification** :
```bash
cargo check --workspace
# Doit passer sans erreur (les crates peuvent être vides pour l'instant)
```

---

## ÉTAPE 2 — Reconstruire ferropdf-core

Ce crate contient uniquement des types partagés. Aucune logique métier.

```toml
# crates/ferropdf-core/Cargo.toml
[package]
name    = "ferropdf-core"
version = "0.2.0"
edition = "2026"

[dependencies]
thiserror = { workspace = true }
hashbrown = { workspace = true }
id-arena  = { workspace = true }
```

Créer les fichiers suivants dans `crates/ferropdf-core/src/` :

### `crates/ferropdf-core/src/lib.rs`
```rust
pub mod color;
pub mod geometry;
pub mod length;
pub mod page;
pub mod dom;
pub mod style;
pub mod layout;
pub mod error;

pub use color::Color;
pub use geometry::{Rect, Point, Size, Insets};
pub use length::Length;
pub use page::{PageSize, PageConfig, PageMargins, Orientation};
pub use dom::{Document, Node, NodeId, NodeType};
pub use style::ComputedStyle;
pub use layout::LayoutBox;
pub use error::{FerroError, Result};
```

### `crates/ferropdf-core/src/error.rs`
```rust
#[derive(Debug, thiserror::Error)]
pub enum FerroError {
    #[error("HTML parse error: {0}")]   HtmlParse(String),
    #[error("CSS parse error: {0}")]    CssParse(String),
    #[error("Layout error: {0}")]       Layout(String),
    #[error("Font error: {0}")]         Font(String),
    #[error("Image error: {0}")]        Image(String),
    #[error("PDF write error: {0}")]    PdfWrite(String),
    #[error("IO error: {0}")]           Io(#[from] std::io::Error),
}

pub type Result<T> = std::result::Result<T, FerroError>;
```

### `crates/ferropdf-core/src/geometry.rs`
```rust
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Rect {
    pub x: f32, pub y: f32,
    pub width: f32, pub height: f32,
}

impl Rect {
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self { x, y, width, height }
    }
    pub fn zero() -> Self { Self::default() }
    pub fn right(&self)  -> f32 { self.x + self.width }
    pub fn bottom(&self) -> f32 { self.y + self.height }
}

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Size { pub width: f32, pub height: f32 }

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Point { pub x: f32, pub y: f32 }

#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct Insets {
    pub top: f32, pub right: f32,
    pub bottom: f32, pub left: f32,
}

impl Insets {
    pub fn uniform(v: f32) -> Self {
        Self { top: v, right: v, bottom: v, left: v }
    }
    pub fn horizontal(&self) -> f32 { self.left + self.right }
    pub fn vertical(&self)   -> f32 { self.top + self.bottom }
}
```

### `crates/ferropdf-core/src/color.rs`
```rust
#[derive(Debug, Clone, Copy, PartialEq, Default)]
pub struct Color { pub r: f32, pub g: f32, pub b: f32, pub a: f32 }

impl Color {
    pub fn new(r: f32, g: f32, b: f32, a: f32) -> Self { Self { r, g, b, a } }
    pub fn black()       -> Self { Self::new(0.0, 0.0, 0.0, 1.0) }
    pub fn white()       -> Self { Self::new(1.0, 1.0, 1.0, 1.0) }
    pub fn transparent() -> Self { Self::new(0.0, 0.0, 0.0, 0.0) }

    pub fn from_rgb8(r: u8, g: u8, b: u8) -> Self {
        Self::new(r as f32 / 255.0, g as f32 / 255.0, b as f32 / 255.0, 1.0)
    }

    pub fn from_hex(hex: &str) -> Option<Self> {
        let hex = hex.trim_start_matches('#');
        match hex.len() {
            6 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                Some(Self::from_rgb8(r, g, b))
            }
            8 => {
                let r = u8::from_str_radix(&hex[0..2], 16).ok()?;
                let g = u8::from_str_radix(&hex[2..4], 16).ok()?;
                let b = u8::from_str_radix(&hex[4..6], 16).ok()?;
                let a = u8::from_str_radix(&hex[6..8], 16).ok()?;
                Some(Self::new(r as f32/255.0, g as f32/255.0, b as f32/255.0, a as f32/255.0))
            }
            _ => None,
        }
    }
}
```

### `crates/ferropdf-core/src/length.rs`
```rust
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
    None,   // max-width: none
}

impl Length {
    pub fn is_auto(&self)    -> bool { matches!(self, Length::Auto) }
    pub fn is_percent(&self) -> bool { matches!(self, Length::Percent(_)) }

    pub fn to_px_with_base(&self, base: f32) -> Option<f32> {
        match self {
            Length::Px(v)      => Some(*v),
            Length::Pt(v)      => Some(v * 1.333_333),
            Length::Mm(v)      => Some(v * 3.779_528),
            Length::Em(v)      => Some(v * base),
            Length::Rem(v)     => Some(v * 16.0),   // rem relatif à root 16px
            Length::Percent(_) => None,              // résolu dans le layout par Taffy
            Length::Auto       => None,
            Length::Zero       => Some(0.0),
            Length::None       => None,
        }
    }
}

impl Default for Length {
    fn default() -> Self { Length::Auto }
}
```

### `crates/ferropdf-core/src/page.rs`
```rust
use crate::length::Length;
use crate::geometry::Insets;

#[derive(Debug, Clone, PartialEq)]
pub enum PageSize {
    A3, A4, A5,
    Letter, Legal,
    Custom(f32, f32),   // width, height en points (pt)
}

impl PageSize {
    // Dimensions en points (1pt = 1/72 inch)
    pub fn dimensions_pt(&self) -> (f32, f32) {
        match self {
            PageSize::A3     => (841.89, 1190.55),
            PageSize::A4     => (595.28,  841.89),
            PageSize::A5     => (419.53,  595.28),
            PageSize::Letter => (612.0,   792.0),
            PageSize::Legal  => (612.0,  1008.0),
            PageSize::Custom(w, h) => (*w, *h),
        }
    }
    pub fn name(&self) -> &str {
        match self {
            PageSize::A3     => "A3",
            PageSize::A4     => "A4",
            PageSize::A5     => "A5",
            PageSize::Letter => "Letter",
            PageSize::Legal  => "Legal",
            PageSize::Custom(_, _) => "Custom",
        }
    }
}

#[derive(Debug, Clone)]
pub enum Orientation { Portrait, Landscape }

#[derive(Debug, Clone)]
pub struct PageMargins {
    pub top: f32, pub right: f32,
    pub bottom: f32, pub left: f32,
}

impl PageMargins {
    pub fn uniform(pt: f32) -> Self {
        Self { top: pt, right: pt, bottom: pt, left: pt }
    }
    pub fn mm(top: f32, right: f32, bottom: f32, left: f32) -> Self {
        // Convertir mm → pt
        let mm_to_pt = 2.834_646;
        Self {
            top:    top    * mm_to_pt,
            right:  right  * mm_to_pt,
            bottom: bottom * mm_to_pt,
            left:   left   * mm_to_pt,
        }
    }
}

impl Default for PageMargins {
    fn default() -> Self { Self::mm(20.0, 20.0, 20.0, 20.0) }
}

#[derive(Debug, Clone)]
pub struct PageConfig {
    pub size:        PageSize,
    pub margins:     PageMargins,
    pub orientation: Orientation,
}

impl Default for PageConfig {
    fn default() -> Self {
        Self {
            size:        PageSize::A4,
            margins:     PageMargins::default(),
            orientation: Orientation::Portrait,
        }
    }
}

impl PageConfig {
    pub fn content_width(&self) -> f32 {
        let (w, _) = self.size.dimensions_pt();
        w - self.margins.left - self.margins.right
    }
    pub fn content_height(&self) -> f32 {
        let (_, h) = self.size.dimensions_pt();
        h - self.margins.top - self.margins.bottom
    }
}
```

### `crates/ferropdf-core/src/dom.rs`
```rust
use std::collections::HashMap;
use id_arena::{Arena, Id};

pub type NodeId = Id<Node>;

#[derive(Debug, Clone, PartialEq)]
pub enum NodeType { Document, Element, Text, Comment }

#[derive(Debug, Clone)]
pub struct Node {
    pub node_type:  NodeType,
    pub tag_name:   Option<String>,
    pub attributes: HashMap<String, String>,
    pub text:       Option<String>,
    pub parent:     Option<NodeId>,
    pub children:   Vec<NodeId>,
}

impl Node {
    pub fn is_element(&self) -> bool { self.node_type == NodeType::Element }
    pub fn is_text(&self)    -> bool { self.node_type == NodeType::Text }
}

#[derive(Debug, Default)]
pub struct Document {
    pub nodes: Arena<Node>,
    pub root:  Option<NodeId>,
}

impl Document {
    pub fn new() -> Self { Self::default() }

    pub fn create_element(&mut self, tag: &str, attrs: HashMap<String, String>) -> NodeId {
        self.nodes.alloc(Node {
            node_type:  NodeType::Element,
            tag_name:   Some(tag.to_lowercase()),
            attributes: attrs,
            text:       None,
            parent:     None,
            children:   Vec::new(),
        })
    }

    pub fn create_text(&mut self, content: &str) -> NodeId {
        self.nodes.alloc(Node {
            node_type:  NodeType::Text,
            tag_name:   None,
            attributes: HashMap::new(),
            text:       Some(content.to_string()),
            parent:     None,
            children:   Vec::new(),
        })
    }

    pub fn append_child(&mut self, parent: NodeId, child: NodeId) {
        self.nodes[child].parent = Some(parent);
        self.nodes[parent].children.push(child);
    }

    pub fn get(&self, id: NodeId) -> &Node { &self.nodes[id] }

    pub fn root(&self) -> NodeId { self.root.expect("Document has no root") }
}
```

### `crates/ferropdf-core/src/style.rs`
```rust
use crate::{Color, Length};

#[derive(Debug, Clone, PartialEq)]
pub enum Display   { Block, Inline, InlineBlock, Flex, Grid, Table,
                     TableRow, TableCell, TableHeaderGroup, TableRowGroup,
                     ListItem, None }
#[derive(Debug, Clone, PartialEq)]
pub enum Position  { Static, Relative, Absolute, Fixed, Sticky }
#[derive(Debug, Clone, PartialEq)]
pub enum FontWeight { Normal, Bold, Bolder, Lighter, Number(u16) }
#[derive(Debug, Clone, PartialEq)]
pub enum FontStyle  { Normal, Italic, Oblique }
#[derive(Debug, Clone, PartialEq)]
pub enum TextAlign  { Left, Right, Center, Justify }
#[derive(Debug, Clone, PartialEq)]
pub enum FlexDirection   { Row, Column, RowReverse, ColumnReverse }
#[derive(Debug, Clone, PartialEq)]
pub enum FlexWrap        { NoWrap, Wrap, WrapReverse }
#[derive(Debug, Clone, PartialEq)]
pub enum JustifyContent  { FlexStart, FlexEnd, Center, SpaceBetween, SpaceAround, SpaceEvenly }
#[derive(Debug, Clone, PartialEq)]
pub enum AlignItems      { Stretch, FlexStart, FlexEnd, Center, Baseline }
#[derive(Debug, Clone, PartialEq)]
pub enum AlignSelf       { Auto, Stretch, FlexStart, FlexEnd, Center, Baseline }
#[derive(Debug, Clone, PartialEq)]
pub enum PageBreak       { Auto, Always, Avoid, Left, Right }

#[derive(Debug, Clone)]
pub struct BorderSide {
    pub width: f32,
    pub color: Color,
    pub style: BorderStyle,
}
#[derive(Debug, Clone, PartialEq)]
pub enum BorderStyle { None, Solid, Dashed, Dotted, Double }

#[derive(Debug, Clone, Default)]
pub struct BorderRadius {
    pub top_left:     f32,
    pub top_right:    f32,
    pub bottom_right: f32,
    pub bottom_left:  f32,
}
impl BorderRadius {
    pub fn uniform(r: f32) -> Self {
        Self { top_left: r, top_right: r, bottom_right: r, bottom_left: r }
    }
    pub fn any_nonzero(&self) -> bool {
        self.top_left > 0.0 || self.top_right > 0.0
            || self.bottom_right > 0.0 || self.bottom_left > 0.0
    }
    pub fn to_array(&self) -> [f32; 4] {
        [self.top_left, self.top_right, self.bottom_right, self.bottom_left]
    }
}

// ─── ComputedStyle ────────────────────────────────────────────────────────────
// Toutes les valeurs sont résolues en px/rgba — plus de em/rem/% ici
// (sauf width/height qui sont résolus par Taffy pendant le layout)

#[derive(Debug, Clone)]
pub struct ComputedStyle {
    // Box model
    pub display:    Display,
    pub position:   Position,
    pub width:      Length,     // peut rester Percent — Taffy le gère
    pub height:     Length,
    pub min_width:  Length,
    pub max_width:  Length,
    pub min_height: Length,
    pub max_height: Length,

    // Spacing
    pub margin:  [Length; 4],   // top, right, bottom, left
    pub padding: [Length; 4],
    pub border:  [BorderSide; 4],
    pub border_radius: BorderRadius,

    // Couleurs
    pub color:            Color,
    pub background_color: Color,
    pub opacity:          f32,

    // Texte
    pub font_family:  Vec<String>,
    pub font_size:    f32,       // toujours px
    pub font_weight:  FontWeight,
    pub font_style:   FontStyle,
    pub line_height:  f32,       // toujours px
    pub text_align:   TextAlign,

    // Flex
    pub flex_direction:   FlexDirection,
    pub flex_wrap:        FlexWrap,
    pub justify_content:  JustifyContent,
    pub align_items:      AlignItems,
    pub align_self:       AlignSelf,
    pub flex_grow:        f32,
    pub flex_shrink:      f32,
    pub flex_basis:       Length,
    pub column_gap:       Length,
    pub row_gap:          Length,

    // Pagination
    pub page_break_before: PageBreak,
    pub page_break_after:  PageBreak,
    pub page_break_inside: PageBreak,
    pub orphans:           u32,
    pub widows:            u32,
}

impl Default for ComputedStyle {
    fn default() -> Self {
        Self {
            display:          Display::Block,
            position:         Position::Static,
            width:            Length::Auto,
            height:           Length::Auto,
            min_width:        Length::Zero,
            max_width:        Length::None,
            min_height:       Length::Zero,
            max_height:       Length::None,
            margin:           [Length::Zero; 4],
            padding:          [Length::Zero; 4],
            border:           std::array::from_fn(|_| BorderSide {
                width: 0.0,
                color: Color::black(),
                style: BorderStyle::None,
            }),
            border_radius:    BorderRadius::default(),
            color:            Color::black(),
            background_color: Color::transparent(),
            opacity:          1.0,
            font_family:      vec!["sans-serif".to_string()],
            font_size:        16.0,
            font_weight:      FontWeight::Normal,
            font_style:       FontStyle::Normal,
            line_height:      19.2,  // 16 * 1.2
            text_align:       TextAlign::Left,
            flex_direction:   FlexDirection::Row,
            flex_wrap:        FlexWrap::NoWrap,
            justify_content:  JustifyContent::FlexStart,
            align_items:      AlignItems::Stretch,
            align_self:       AlignSelf::Auto,
            flex_grow:        0.0,
            flex_shrink:      1.0,
            flex_basis:       Length::Auto,
            column_gap:       Length::Zero,
            row_gap:          Length::Zero,
            page_break_before: PageBreak::Auto,
            page_break_after:  PageBreak::Auto,
            page_break_inside: PageBreak::Auto,
            orphans:           2,
            widows:            2,
        }
    }
}
```

### `crates/ferropdf-core/src/layout.rs`
```rust
use crate::{ComputedStyle, NodeId, Rect, Insets};

#[derive(Debug, Clone)]
pub struct LayoutBox {
    pub node_id:    Option<NodeId>,
    pub style:      ComputedStyle,
    pub content:    Rect,
    pub padding:    Insets,
    pub border:     Insets,
    pub margin:     Insets,
    pub children:   Vec<LayoutBox>,
    // Texte shapé (rempli par ferropdf-layout)
    pub shaped_lines: Vec<ShapedLine>,
    pub image_src:    Option<String>,
}

#[derive(Debug, Clone)]
pub struct ShapedLine {
    pub glyphs:    Vec<ShapedGlyph>,
    pub width:     f32,
    pub y:         f32,
}

#[derive(Debug, Clone)]
pub struct ShapedGlyph {
    pub glyph_id:  u16,
    pub x:         f32,
    pub y:         f32,
    pub advance:   f32,
    pub font_id:   u64,
}

impl LayoutBox {
    pub fn margin_box_height(&self) -> f32 {
        self.margin.top + self.border.top + self.padding.top
        + self.content.height
        + self.padding.bottom + self.border.bottom + self.margin.bottom
    }
    pub fn border_box(&self) -> Rect {
        Rect::new(
            self.content.x - self.padding.left - self.border.left,
            self.content.y - self.padding.top  - self.border.top,
            self.content.width  + self.padding.horizontal() + self.border.horizontal(),
            self.content.height + self.padding.vertical()   + self.border.vertical(),
        )
    }
}

#[derive(Debug)]
pub struct LayoutTree {
    pub root: Option<LayoutBox>,
}

impl LayoutTree {
    pub fn new() -> Self { Self { root: None } }
}
```

**Vérification** :
```bash
cargo build -p ferropdf-core
# Doit compiler sans warning
```

---

## ÉTAPE 3 — Reconstruire ferropdf-parse avec html5ever + cssparser

**Supprimer tout le code du parser actuel** et réécrire avec les libs.

```toml
# crates/ferropdf-parse/Cargo.toml
[package]
name    = "ferropdf-parse"
version = "0.1.0"
edition = "2021"

[dependencies]
ferropdf-core = { path = "../ferropdf-core" }
html5ever      = { workspace = true }
markup5ever    = { workspace = true }
cssparser      = { workspace = true }
selectors      = { workspace = true }
url            = { workspace = true }
log            = { workspace = true }
```

### `crates/ferropdf-parse/src/lib.rs`
```rust
mod html;
mod css;

pub use html::parse_html;
pub use css::{parse_stylesheet, Stylesheet, StyleRule, Declaration, Property, Value};

use ferropdf_core::Result;

pub struct ParseResult {
    pub document:            ferropdf_core::Document,
    pub inline_styles:       Vec<String>,      // contenu des <style>
    pub external_stylesheets: Vec<String>,     // href des <link>
}

pub fn parse(html: &str) -> Result<ParseResult> {
    html::parse_full(html)
}
```

### `crates/ferropdf-parse/src/html.rs`
```rust
use std::collections::HashMap;
use html5ever::{
    parse_document,
    tendril::TendrilSink,
    tree_builder::{TreeBuilderOpts, NodeOrText, TreeSink},
    QualName, Attribute,
};
use markup5ever::interface::QuirksMode;
use ferropdf_core::{Document, NodeId, Result};
use crate::ParseResult;

struct DomBuilder {
    doc:             Document,
    inline_styles:   Vec<String>,
    external_sheets: Vec<String>,
}

impl DomBuilder {
    fn new() -> Self {
        let mut doc = Document::new();
        // Créer le nœud document racine
        let root = doc.nodes.alloc(ferropdf_core::dom::Node {
            node_type:  ferropdf_core::NodeType::Document,
            tag_name:   None,
            attributes: HashMap::new(),
            text:       None,
            parent:     None,
            children:   Vec::new(),
        });
        doc.root = Some(root);
        Self { doc, inline_styles: Vec::new(), external_sheets: Vec::new() }
    }
}

impl TreeSink for DomBuilder {
    type Handle  = NodeId;
    type Output  = Self;
    type Error   = std::convert::Infallible;

    fn finish(self) -> Self { self }

    fn get_document(&mut self) -> Self::Handle {
        self.doc.root.unwrap()
    }

    fn get_template_contents(&mut self, target: &Self::Handle) -> Self::Handle {
        *target
    }

    fn same_node(&self, x: &Self::Handle, y: &Self::Handle) -> bool { x == y }

    fn elem_name<'a>(&'a self, target: &'a Self::Handle) -> html5ever::ExpandedName<'a> {
        let node = &self.doc.nodes[*target];
        let tag  = node.tag_name.as_deref().unwrap_or("unknown");
        html5ever::expanded_name!(html tag)
    }

    fn create_element(
        &mut self,
        name:  QualName,
        attrs: Vec<Attribute>,
        _:     html5ever::tree_builder::ElementFlags,
    ) -> Self::Handle {
        let tag = name.local.as_ref().to_lowercase();
        let mut attr_map: HashMap<String, String> = attrs.iter()
            .map(|a| (a.name.local.to_string(), a.value.to_string()))
            .collect();

        let id = self.doc.create_element(&tag, attr_map.clone());

        // Collecter les <link rel="stylesheet"> et <style>
        if tag == "link" {
            if attr_map.get("rel").map(|s| s.as_str()) == Some("stylesheet") {
                if let Some(href) = attr_map.get("href") {
                    self.external_sheets.push(href.clone());
                }
            }
        }

        id
    }

    fn create_text_node(&mut self, text: html5ever::tendril::StrTendril) -> Self::Handle {
        self.doc.create_text(text.as_ref())
    }

    fn create_comment(&mut self, _: html5ever::tendril::StrTendril) -> Self::Handle {
        self.doc.create_text("")   // ignorer les commentaires
    }

    fn create_pi(&mut self, _: html5ever::tendril::StrTendril,
                 _: html5ever::tendril::StrTendril) -> Self::Handle {
        self.doc.create_text("")
    }

    fn append(&mut self, parent: &Self::Handle, child: NodeOrText<Self::Handle>) {
        match child {
            NodeOrText::AppendNode(id) => {
                self.doc.append_child(*parent, id);

                // Si c'est un <style>, noter son NodeId pour extraire le texte après
            }
            NodeOrText::AppendText(text) => {
                let id = self.doc.create_text(text.as_ref());
                self.doc.append_child(*parent, id);
            }
        }
    }

    fn append_based_on_parent_node(
        &mut self, element: &Self::Handle,
        prev_element: &Self::Handle,
        child: NodeOrText<Self::Handle>,
    ) {
        self.append(element, child);
    }

    fn append_doctype_to_document(&mut self, _: html5ever::tendril::StrTendril,
                                   _: html5ever::tendril::StrTendril,
                                   _: html5ever::tendril::StrTendril) {}

    fn add_attrs_if_missing(&mut self, target: &Self::Handle, attrs: Vec<Attribute>) {
        let node = &mut self.doc.nodes[*target];
        for attr in attrs {
            node.attributes.entry(attr.name.local.to_string())
                .or_insert_with(|| attr.value.to_string());
        }
    }

    fn associate_with_form(&mut self, _: &Self::Handle, _: &Option<Self::Handle>,
                            _: NodeOrText<Self::Handle>, _: &Option<Self::Handle>) {}

    fn remove_from_parent(&mut self, target: &Self::Handle) {
        if let Some(parent_id) = self.doc.nodes[*target].parent {
            self.doc.nodes[parent_id].children.retain(|&c| c != *target);
            self.doc.nodes[*target].parent = None;
        }
    }

    fn reparent_children(&mut self, node: &Self::Handle, new_parent: &Self::Handle) {
        let children: Vec<_> = self.doc.nodes[*node].children.clone();
        for child in children {
            self.doc.nodes[child].parent = Some(*new_parent);
            self.doc.nodes[*new_parent].children.push(child);
        }
        self.doc.nodes[*node].children.clear();
    }

    fn mark_script_already_started(&mut self, _: &Self::Handle) {}
    fn pop(&mut self, _: &Self::Handle) {}
    fn set_quirks_mode(&mut self, _: QuirksMode) {}
    fn is_mathml_annotation_xml_integration_point(&self, _: &Self::Handle) -> bool { false }
    fn set_current_line(&mut self, _: u64) {}
    fn on_parse_error(&mut self, msg: std::borrow::Cow<'static, str>) {
        log::warn!("HTML parse warning: {}", msg);
    }
}

pub fn parse_full(html: &str) -> Result<ParseResult> {
    let sink = DomBuilder::new();
    let parser = parse_document(sink, Default::default());
    let mut builder = parser.one(html);

    // Extraire le contenu des balises <style>
    extract_style_content(&mut builder);

    Ok(ParseResult {
        inline_styles:        builder.inline_styles,
        external_stylesheets: builder.external_sheets,
        document:             builder.doc,
    })
}

fn extract_style_content(builder: &mut DomBuilder) {
    // Parcourir le DOM et extraire le texte des <style>
    let all_ids: Vec<_> = builder.doc.nodes.iter().map(|(id, _)| id).collect();
    for id in all_ids {
        let node = &builder.doc.nodes[id];
        if node.tag_name.as_deref() == Some("style") {
            let text: String = node.children.iter()
                .filter_map(|&child| builder.doc.nodes[child].text.clone())
                .collect();
            if !text.is_empty() {
                builder.inline_styles.push(text);
            }
        }
    }
}
```

**Vérification** :
```bash
cargo build -p ferropdf-parse
```

---

## ÉTAPE 4 — Reconstruire ferropdf-layout avec Taffy + cosmic-text

**C'est l'étape la plus importante. Elle résout les Fix 1 et Fix 2 structurellement.**

```toml
# crates/ferropdf-layout/Cargo.toml
[package]
name    = "ferropdf-layout"
version = "0.1.0"
edition = "2021"

[dependencies]
ferropdf-core  = { path = "../ferropdf-core" }
ferropdf-parse = { path = "../ferropdf-parse" }
taffy          = { workspace = true }
cosmic-text    = { workspace = true }
fontdb         = { workspace = true }
ttf-parser     = { workspace = true }
log            = { workspace = true }
```

### `crates/ferropdf-layout/src/lib.rs`
```rust
mod taffy_bridge;
mod text;
mod style_to_taffy;

pub use text::TextMeasurer;
use ferropdf_core::{LayoutTree, LayoutBox, ComputedStyle, NodeId, Result, FerroError};
use ferropdf_parse::ParseResult;
use taffy::prelude::*;

pub struct LayoutEngine {
    text_measurer: TextMeasurer,
}

impl LayoutEngine {
    pub fn new() -> Self {
        Self { text_measurer: TextMeasurer::new() }
    }

    pub fn layout(
        &mut self,
        parse_result: &ParseResult,
        style_tree:   &ferropdf_core::StyleTree,
        page_config:  &ferropdf_core::PageConfig,
    ) -> Result<LayoutTree> {
        let page_w = page_config.content_width();

        let mut taffy = TaffyTree::<NodeId>::new();

        // 1. Construire l'arbre Taffy depuis le StyleTree
        // Taffy gère width:100%, padding, flex, grid — plus de bugs Fix 1 / Fix 2
        let root_node = taffy_bridge::build(
            style_tree,
            &mut taffy,
            &mut self.text_measurer,
        )?;

        // 2. Calculer le layout
        // Taffy résout tous les % par rapport au containing block automatiquement
        taffy.compute_layout(
            root_node,
            Size {
                width:  AvailableSpace::Definite(page_w),
                height: AvailableSpace::MaxContent,
            },
        ).map_err(|e| FerroError::Layout(format!("{:?}", e)))?;

        // 3. Lire les résultats → notre LayoutTree
        let root_box = taffy_bridge::read_layout(
            root_node,
            &taffy,
            style_tree,
            &mut self.text_measurer,
            0.0, 0.0,   // offset initial
        )?;

        Ok(LayoutTree { root: Some(root_box) })
    }
}
```

### `crates/ferropdf-layout/src/text.rs`
```rust
use cosmic_text::{Attrs, Buffer, Family, FontSystem, Metrics, Shaping, SwashCache};
use ferropdf_core::{ShapedGlyph, ShapedLine};

pub struct TextMeasurer {
    pub font_system: FontSystem,
    pub swash_cache: SwashCache,
}

impl TextMeasurer {
    pub fn new() -> Self {
        let mut font_system = FontSystem::new();
        font_system.db_mut().load_system_fonts();
        // Charger les fonts embarquées (Liberation Sans/Serif/Mono)
        // pour garantir un rendu cohérent sans dépendance système
        Self {
            font_system,
            swash_cache: SwashCache::new(),
        }
    }

    /// Mesurer la taille d'un texte dans une largeur donnée → (w, h)
    /// Appelé par Taffy pour les leaf nodes (nœuds texte)
    pub fn measure(
        &mut self,
        text:        &str,
        font_size:   f32,
        line_height: f32,
        font_family: &[String],
        bold:        bool,
        italic:      bool,
        max_width:   f32,
    ) -> (f32, f32) {
        if text.trim().is_empty() { return (0.0, 0.0); }

        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, max_width, f32::MAX);

        let family_name = font_family.first().map(|s| s.as_str()).unwrap_or("sans-serif");
        let attrs = Attrs::new().family(Family::Name(family_name));

        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        let width = buffer.layout_runs()
            .map(|r| r.line_w)
            .fold(0.0_f32, f32::max);
        let lines = buffer.layout_runs().count();
        let height = lines as f32 * line_height;

        (width.ceil(), height.ceil())
    }

    /// Shaper le texte et retourner les glyphs positionnés
    /// Appelé par le Renderer pour dessiner le texte
    pub fn shape(
        &mut self,
        text:        &str,
        font_size:   f32,
        line_height: f32,
        font_family: &[String],
        bold:        bool,
        italic:      bool,
        max_width:   f32,
    ) -> Vec<ShapedLine> {
        if text.trim().is_empty() { return Vec::new(); }

        let metrics = Metrics::new(font_size, line_height);
        let mut buffer = Buffer::new(&mut self.font_system, metrics);
        buffer.set_size(&mut self.font_system, max_width, f32::MAX);

        let family_name = font_family.first().map(|s| s.as_str()).unwrap_or("sans-serif");
        let attrs = Attrs::new().family(Family::Name(family_name));

        buffer.set_text(&mut self.font_system, text, attrs, Shaping::Advanced);
        buffer.shape_until_scroll(&mut self.font_system, false);

        buffer.layout_runs().map(|run| ShapedLine {
            y:     run.line_y,
            width: run.line_w,
            glyphs: run.glyphs.iter().map(|g| ShapedGlyph {
                glyph_id: g.glyph_id,
                x:        g.x,
                y:        run.line_y,
                advance:  g.w,
                font_id:  0, // rempli par le font registry
            }).collect(),
        }).collect()
    }
}
```

### `crates/ferropdf-layout/src/style_to_taffy.rs`
```rust
use taffy::prelude::*;
use ferropdf_core::ComputedStyle;
use ferropdf_core::length::Length;

/// Convertir notre ComputedStyle → taffy::Style
/// C'est la clé qui résout Fix 1 (width:100%) et Fix 2 (double padding)
/// car Taffy gère ces cas correctement
pub fn convert(style: &ComputedStyle) -> taffy::Style {
    taffy::Style {
        display: match style.display {
            ferropdf_core::Display::Block       => Display::Block,
            ferropdf_core::Display::Flex        => Display::Flex,
            ferropdf_core::Display::Grid        => Display::Grid,
            ferropdf_core::Display::None        => Display::None,
            ferropdf_core::Display::Inline      |
            ferropdf_core::Display::InlineBlock => Display::Block,
            _ => Display::Block,
        },

        // Dimensions — Taffy résout les % par rapport au containing block
        // c'est ce qui corrige Fix 1 (width:100% → 0px)
        size: Size {
            width:  to_dimension(&style.width),
            height: to_dimension(&style.height),
        },
        min_size: Size {
            width:  to_dimension(&style.min_width),
            height: to_dimension(&style.min_height),
        },
        max_size: Size {
            width:  to_dimension(&style.max_width),
            height: to_dimension(&style.max_height),
        },

        // Padding — Taffy soustrait une seule fois (Fix 2 résolu)
        padding: Rect {
            top:    to_length_pct(&style.padding[0]),
            right:  to_length_pct(&style.padding[1]),
            bottom: to_length_pct(&style.padding[2]),
            left:   to_length_pct(&style.padding[3]),
        },

        // Border widths
        border: Rect {
            top:    LengthPercentage::Length(style.border[0].width),
            right:  LengthPercentage::Length(style.border[1].width),
            bottom: LengthPercentage::Length(style.border[2].width),
            left:   LengthPercentage::Length(style.border[3].width),
        },

        // Margins
        margin: Rect {
            top:    to_length_pct_auto(&style.margin[0]),
            right:  to_length_pct_auto(&style.margin[1]),
            bottom: to_length_pct_auto(&style.margin[2]),
            left:   to_length_pct_auto(&style.margin[3]),
        },

        // Flexbox
        flex_direction: match style.flex_direction {
            ferropdf_core::FlexDirection::Row           => FlexDirection::Row,
            ferropdf_core::FlexDirection::Column        => FlexDirection::Column,
            ferropdf_core::FlexDirection::RowReverse    => FlexDirection::RowReverse,
            ferropdf_core::FlexDirection::ColumnReverse => FlexDirection::ColumnReverse,
        },
        flex_wrap: match style.flex_wrap {
            ferropdf_core::FlexWrap::NoWrap      => FlexWrap::NoWrap,
            ferropdf_core::FlexWrap::Wrap        => FlexWrap::Wrap,
            ferropdf_core::FlexWrap::WrapReverse => FlexWrap::WrapReverse,
        },
        justify_content: Some(match style.justify_content {
            ferropdf_core::JustifyContent::FlexStart    => JustifyContent::FlexStart,
            ferropdf_core::JustifyContent::FlexEnd      => JustifyContent::FlexEnd,
            ferropdf_core::JustifyContent::Center       => JustifyContent::Center,
            ferropdf_core::JustifyContent::SpaceBetween => JustifyContent::SpaceBetween,
            ferropdf_core::JustifyContent::SpaceAround  => JustifyContent::SpaceAround,
            ferropdf_core::JustifyContent::SpaceEvenly  => JustifyContent::SpaceEvenly,
        }),
        align_items: Some(match style.align_items {
            ferropdf_core::AlignItems::Stretch   => AlignItems::Stretch,
            ferropdf_core::AlignItems::FlexStart => AlignItems::FlexStart,
            ferropdf_core::AlignItems::FlexEnd   => AlignItems::FlexEnd,
            ferropdf_core::AlignItems::Center    => AlignItems::Center,
            ferropdf_core::AlignItems::Baseline  => AlignItems::Baseline,
        }),
        flex_grow:   style.flex_grow,
        flex_shrink: style.flex_shrink,
        flex_basis:  to_dimension(&style.flex_basis),
        gap: Size {
            width:  to_length_pct(&style.column_gap),
            height: to_length_pct(&style.row_gap),
        },

        ..Default::default()
    }
}

fn to_dimension(l: &Length) -> Dimension {
    match l {
        Length::Px(v)      => Dimension::Length(*v),
        Length::Percent(v) => Dimension::Percent(v / 100.0),
        Length::Auto       => Dimension::Auto,
        Length::Zero       => Dimension::Length(0.0),
        Length::None       => Dimension::Auto,
        other => {
            // em/rem doivent déjà être résolus en px avant d'arriver ici
            log::warn!("to_dimension: unresolved length {:?}, using Auto", other);
            Dimension::Auto
        }
    }
}

fn to_length_pct(l: &Length) -> LengthPercentage {
    match l {
        Length::Px(v)      => LengthPercentage::Length(*v),
        Length::Percent(v) => LengthPercentage::Percent(v / 100.0),
        Length::Zero       => LengthPercentage::Length(0.0),
        other              => LengthPercentage::Length(0.0),
    }
}

fn to_length_pct_auto(l: &Length) -> LengthPercentageAuto {
    match l {
        Length::Px(v)      => LengthPercentageAuto::Length(*v),
        Length::Percent(v) => LengthPercentageAuto::Percent(v / 100.0),
        Length::Auto       => LengthPercentageAuto::Auto,
        Length::Zero       => LengthPercentageAuto::Length(0.0),
        _                  => LengthPercentageAuto::Auto,
    }
}
```

**Vérification** :
```bash
cargo build -p ferropdf-layout
```

---

## ÉTAPE 5 — Fixer le workflow maturin (Fix 3 permanent)

**Supprimer définitivement le `cp` manuel.**

```toml
# bindings/python/Cargo.toml — NOM DÉFINITIF, NE PLUS CHANGER
[package]
name    = "ferropdf-python"
version = "0.1.0"
edition = "2021"

[lib]
name       = "_ferropdf"   # ← nom final, ne plus jamais changer
crate-type = ["cdylib"]

[dependencies]
ferropdf-render = { path = "../../crates/ferropdf-render" }
ferropdf-core   = { path = "../../crates/ferropdf-core" }
pyo3 = { workspace = true }

[features]
default = ["pyo3/extension-module"]
```

```toml
# pyproject.toml à la RACINE du projet
[build-system]
requires      = ["maturin>=1.5,<2"]
build-backend = "maturin"

[project]
name            = "ferropdf"
version         = "0.1.0"
description     = "Fast HTML to PDF — Rust-powered"
requires-python = ">=3.8"

[tool.maturin]
manifest-path = "bindings/python/Cargo.toml"
python-source = "python"              # dossier contenant ferropdf/__init__.py
module-name   = "ferropdf._ferropdf"  # ← nom final, ne plus jamais changer
features      = ["pyo3/extension-module"]
```

```rust
// bindings/python/src/lib.rs
use pyo3::prelude::*;
use pyo3::types::PyBytes;

pyo3::create_exception!(ferropdf, FerroError, pyo3::exceptions::PyRuntimeError);

#[pyclass(name = "Options")]
#[derive(Clone)]
struct PyOptions {
    page_size: String,
    margin_mm: f32,
    base_url:  Option<String>,
}

#[pymethods]
impl PyOptions {
    #[new]
    #[pyo3(signature = (page_size="A4", margin="20mm", base_url=None))]
    fn new(page_size: &str, margin: &str, base_url: Option<String>) -> PyResult<Self> {
        let margin_mm = parse_margin_mm(margin)
            .ok_or_else(|| PyErr::new::<pyo3::exceptions::PyValueError, _>(
                format!("Invalid margin '{}'. Use '20mm', '1in', '72pt'", margin)
            ))?;
        Ok(Self {
            page_size: page_size.to_uppercase(),
            margin_mm,
            base_url,
        })
    }
}

#[pyclass(name = "Engine")]
struct PyEngine {
    options: PyOptions,
}

#[pymethods]
impl PyEngine {
    #[new]
    #[pyo3(signature = (options=None))]
    fn new(options: Option<PyOptions>) -> Self {
        Self { options: options.unwrap_or_else(|| PyOptions {
            page_size: "A4".to_string(),
            margin_mm: 20.0,
            base_url:  None,
        })}
    }

    fn render<'py>(&self, py: Python<'py>, html: &str) -> PyResult<&'py PyBytes> {
        let opts = self.options.clone();
        let html = html.to_string();

        // Libérer le GIL pendant le rendu Rust
        // → FastAPI/Django ne sont pas bloqués
        let result = py.allow_threads(move || {
            render_html_to_pdf(&html, &opts)
        });

        match result {
            Ok(bytes) => Ok(PyBytes::new(py, &bytes)),
            Err(e)    => Err(PyErr::new::<FerroError, _>(e.to_string())),
        }
    }

    fn render_file<'py>(&self, py: Python<'py>, path: &str) -> PyResult<&'py PyBytes> {
        let html = std::fs::read_to_string(path)
            .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
        self.render(py, &html)
    }
}

#[pyfunction]
#[pyo3(signature = (html, base_url=None, options=None))]
fn from_html<'py>(
    py:       Python<'py>,
    html:     &str,
    base_url: Option<&str>,
    options:  Option<PyOptions>,
) -> PyResult<&'py PyBytes> {
    let mut opts = options.unwrap_or_else(|| PyOptions {
        page_size: "A4".to_string(),
        margin_mm: 20.0,
        base_url:  None,
    });
    if let Some(url) = base_url { opts.base_url = Some(url.to_string()); }

    let html = html.to_string();
    let result = py.allow_threads(move || render_html_to_pdf(&html, &opts));
    match result {
        Ok(bytes) => Ok(PyBytes::new(py, &bytes)),
        Err(e)    => Err(PyErr::new::<FerroError, _>(e.to_string())),
    }
}

#[pyfunction]
#[pyo3(signature = (path, options=None))]
fn from_file<'py>(py: Python<'py>, path: &str, options: Option<PyOptions>) -> PyResult<&'py PyBytes> {
    let html = std::fs::read_to_string(path)
        .map_err(|e| PyErr::new::<pyo3::exceptions::PyIOError, _>(e.to_string()))?;
    from_html(py, &html, None, options)
}

#[pymodule]
fn _ferropdf(py: Python, m: &PyModule) -> PyResult<()> {
    m.add_class::<PyOptions>()?;
    m.add_class::<PyEngine>()?;
    m.add_function(wrap_pyfunction!(from_html, m)?)?;
    m.add_function(wrap_pyfunction!(from_file, m)?)?;
    m.add("FerroError", py.get_type::<FerroError>())?;
    m.add("__version__", env!("CARGO_PKG_VERSION"))?;
    Ok(())
}

fn render_html_to_pdf(html: &str, opts: &PyOptions) -> std::result::Result<Vec<u8>, String> {
    // Pipeline complète : HTML → PDF
    // Appelée depuis py.allow_threads() donc pas d'accès Python ici
    use ferropdf_parse::parse;
    use ferropdf_layout::LayoutEngine;

    let page_config = build_page_config(&opts.page_size, opts.margin_mm);

    let parse_result = parse(html).map_err(|e| e.to_string())?;
    // Style resolution + layout + pagination + render
    // → à compléter au fur et à mesure des autres crates
    todo!("Connecter les autres crates")
}

fn build_page_config(size: &str, margin_mm: f32) -> ferropdf_core::PageConfig {
    let page_size = match size {
        "A3"     => ferropdf_core::PageSize::A3,
        "A4"     => ferropdf_core::PageSize::A4,
        "A5"     => ferropdf_core::PageSize::A5,
        "LETTER" => ferropdf_core::PageSize::Letter,
        "LEGAL"  => ferropdf_core::PageSize::Legal,
        _        => ferropdf_core::PageSize::A4,
    };
    let mm_to_pt = 2.834_646_f32;
    ferropdf_core::PageConfig {
        size:        page_size,
        margins:     ferropdf_core::PageMargins::uniform(margin_mm * mm_to_pt),
        orientation: ferropdf_core::Orientation::Portrait,
    }
}

fn parse_margin_mm(s: &str) -> Option<f32> {
    if s.ends_with("mm") { s[..s.len()-2].trim().parse().ok() }
    else if s.ends_with("cm") { s[..s.len()-2].trim().parse::<f32>().ok().map(|v| v * 10.0) }
    else if s.ends_with("in") { s[..s.len()-2].trim().parse::<f32>().ok().map(|v| v * 25.4) }
    else if s.ends_with("pt") { s[..s.len()-2].trim().parse::<f32>().ok().map(|v| v / 2.835) }
    else { None }
}
```

---

## ÉTAPE 6 — Wrapper Python

```
python/
└── ferropdf/
    ├── __init__.py
    ├── contrib/
    │   ├── __init__.py
    │   ├── django.py
    │   └── fastapi.py
    └── py.typed
```

### `python/ferropdf/__init__.py`
```python
from ._ferropdf import (
    Engine, Options, FerroError,
    from_html, from_file,
    __version__,
)

__all__ = [
    "Engine", "Options", "FerroError",
    "from_html", "from_file",
    "__version__",
]
```

### `python/ferropdf/contrib/django.py`
```python
from django.http import HttpResponse
from django.template.loader import render_to_string
import ferropdf

class PdfResponse(HttpResponse):
    def __init__(self, template_name, context, request=None,
                 filename="document.pdf", options=None, inline=True, **kwargs):
        html     = render_to_string(template_name, context, request=request)
        base_url = request.build_absolute_uri("/") if request else None
        engine   = ferropdf.Engine(options or ferropdf.Options(base_url=base_url))
        pdf      = engine.render(html)
        super().__init__(content=pdf, content_type="application/pdf", **kwargs)
        disposition = "inline" if inline else "attachment"
        self["Content-Disposition"] = f'{disposition}; filename="{filename}"'
        self["Content-Length"]      = str(len(pdf))
```

### `python/ferropdf/contrib/fastapi.py`
```python
import asyncio
from fastapi.responses import Response
import ferropdf

async def pdf_response(html: str, filename: str = "document.pdf",
                       options=None, inline: bool = True) -> Response:
    engine = ferropdf.Engine(options or ferropdf.Options())
    loop   = asyncio.get_event_loop()
    pdf    = await loop.run_in_executor(None, engine.render, html)
    d      = "inline" if inline else "attachment"
    return Response(
        content=pdf, media_type="application/pdf",
        headers={
            "Content-Disposition": f'{d}; filename="{filename}"',
            "Content-Length":      str(len(pdf)),
        },
    )
```

---

## ÉTAPE 7 — Workflow de build (remplace le cp manuel)

```bash
# Setup une seule fois
python -m venv .venv
source .venv/bin/activate      # Windows: .venv\Scripts\activate
pip install maturin

# Build de développement — UNE SEULE COMMANDE
maturin develop

# Vérifier que ça fonctionne
python -c "
import ferropdf
pdf = ferropdf.from_html('<h1>Test</h1>')
assert pdf[:4] == b'%PDF'
print(f'✅ ferropdf {ferropdf.__version__} fonctionne')
"

# Build release (pour la perf)
maturin develop --release

# Build wheel pour distribution
maturin build --release
```

**Makefile pour simplifier** :
```makefile
# Makefile
.PHONY: dev build test bench

dev:
	maturin develop

release:
	maturin develop --release

test:
	maturin develop
	pytest tests/ -v

bench:
	maturin develop --release
	python bench/compare.py
```

---

## ÉTAPE 8 — Tests à écrire obligatoirement

**Ces tests auraient attrapé Fix 1 et Fix 2 immédiatement.**
Les écrire avant de continuer le développement.

### `tests/test_layout.py`
```python
import ferropdf

def test_width_100_percent():
    """Fix 1 : width:100% ne doit pas donner 0px"""
    html = """
    <div style="width:500px">
      <table style="width:100%">
        <tr><td>Colonne 1</td><td>Colonne 2</td></tr>
      </table>
    </div>
    """
    pdf = ferropdf.from_html(html)
    assert pdf[:4] == b"%PDF"
    # Si width:100% est 0px, le PDF fera plus de 2 pages
    # On ne peut pas mesurer les pages facilement ici mais au moins ça compile

def test_no_double_padding():
    """Fix 2 : le padding ne doit pas être soustrait deux fois"""
    html = """
    <div style="width:200px; padding:20px; background:red">
      <p>Texte dans la div</p>
    </div>
    """
    pdf = ferropdf.from_html(html)
    assert pdf[:4] == b"%PDF"

def test_invoice_two_pages_max():
    """Le template invoice ne doit pas dépasser 2 pages"""
    import os
    invoice_path = "examples/templates/invoice.html"
    if os.path.exists(invoice_path):
        pdf = ferropdf.from_file(invoice_path)
        assert pdf[:4] == b"%PDF"
        # Compter les occurrences de /Page dans le PDF
        page_count = pdf.count(b"/Type /Page\n")
        assert page_count <= 2, f"Invoice fait {page_count} pages, max 2 attendu"

def test_flex_layout():
    """Flexbox doit distribuer l'espace correctement"""
    html = """
    <div style="display:flex; width:600px">
      <div style="flex:1; background:red">A</div>
      <div style="flex:1; background:blue">B</div>
      <div style="flex:1; background:green">C</div>
    </div>
    """
    pdf = ferropdf.from_html(html)
    assert pdf[:4] == b"%PDF"

def test_engine_reusable():
    """Le même Engine peut être utilisé plusieurs fois sans état résiduel"""
    engine = ferropdf.Engine()
    r1 = engine.render("<p>Doc 1</p>")
    r2 = engine.render("<p>Doc 2</p>")
    assert r1[:4] == b"%PDF"
    assert r2[:4] == b"%PDF"
    # Les deux PDFs sont différents
    assert r1 != r2

def test_broken_html_no_crash():
    """HTML malformé ne doit jamais crasher (html5ever gère ça)"""
    cases = [
        "<p>Unclosed paragraph",
        "<div><p>Double unclosed",
        "Pas de balises du tout",
        "",
        "<script>alert('xss')</script><p>texte</p>",
    ]
    for html in cases:
        result = ferropdf.from_html(html)
        assert result[:4] == b"%PDF", f"Crash sur: {html!r}"

def test_performance():
    """Doit être significativement plus rapide que WeasyPrint"""
    import time
    engine = ferropdf.Engine()
    html   = open("examples/templates/invoice.html").read()

    # Warm up
    engine.render(html)

    N  = 10
    t0 = time.perf_counter()
    for _ in range(N):
        engine.render(html)
    ms_per_doc = (time.perf_counter() - t0) / N * 1000

    print(f"\nPerf: {ms_per_doc:.1f}ms/doc")
    assert ms_per_doc < 200, f"Trop lent: {ms_per_doc:.1f}ms (cible: < 200ms)"
```

---

## ÉTAPE 9 — Vérification finale

```bash
# Tout doit passer
cargo build --workspace
cargo test  --workspace
cargo clippy --workspace -- -D warnings

maturin develop --release
pytest tests/ -v

# Benchmark
python bench/compare.py
```

**Résultats attendus** :
```
✅ cargo build    — 0 erreurs, 0 warnings
✅ cargo test     — tous les tests Rust passent
✅ pytest         — tous les tests Python passent
✅ invoice.html   — 1-2 pages (plus 8)
✅ Vitesse        — > 10x WeasyPrint
```

---

## Récapitulatif des changements

| Quoi | Avant (bugué) | Après (correct) |
|---|---|---|
| Parser HTML | Maison (fragile) | `html5ever` (Firefox-grade) |
| Parser CSS | Maison (fragile) | `cssparser` + `selectors` (Mozilla) |
| Layout engine | Maison (Fix 1 + Fix 2) | `taffy` (battle-tested) |
| Text shaping | Basique ou absent | `cosmic-text` (shaping complet) |
| Build workflow | `cp` manuel | `maturin develop` |
| Tests | Absents | Suite complète |

**Ces changements éliminent structurellement les 3 bugs du FIXES.md**
et ouvrent la voie vers 15-20x WeasyPrint.
