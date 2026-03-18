// =============================================================================
// block_flow.rs — Algorithme de layout Block Flow
// =============================================================================
// Traduit depuis les algorithmes de Blink (BSD licence) :
//   blink/renderer/core/layout/layout_block_flow.cc
//   blink/renderer/core/layout/layout_block.cc
//
// Spec de référence :
//   CSS 2.1 §9.4.1 — Block formatting context
//   CSS 2.1 §8.3.1 — Collapsing margins
//
// Ce module s'exécute APRÈS Taffy comme un post-pass.
// Taffy calcule les dimensions (width, height, padding, border).
// Ce module corrige les positions Y pour le margin collapsing.
// =============================================================================

use ferropdf_core::{LayoutBox, LayoutTree, ComputedStyle, NodeId, FerroError};
use std::collections::HashMap;

// =============================================================================
// STRUCTURES DE DONNÉES
// =============================================================================

/// Représente les marges d'un bloc après résolution du collapsing.
/// Inspiré de MarginInfo dans layout_block_flow.cc de Blink.
#[derive(Debug, Clone, Default)]
pub struct ResolvedMargins {
    pub top: f32,
    pub bottom: f32,
    pub left: f32,
    pub right: f32,
}

/// Contexte de block formatting — stocke l'état courant du layout.
/// Inspiré de BlockFormattingContext dans Blink LayoutNG.
#[derive(Debug)]
struct BlockFormattingContext {
    /// Position Y courante dans le flux (curseur vertical)
    current_y: f32,
    /// Marge bottom du bloc précédent (pour le margin collapsing)
    pending_margin_bottom: f32,
    /// Largeur du containing block
    containing_width: f32,
}

impl BlockFormattingContext {
    fn new(containing_width: f32) -> Self {
        Self {
            current_y: 0.0,
            pending_margin_bottom: 0.0,
            containing_width,
        }
    }
}

// =============================================================================
// POINT D'ENTRÉE PRINCIPAL
// =============================================================================

/// Applique le block flow layout sur un LayoutTree existant.
/// Corrige les positions Y après le passage de Taffy.
///
/// # Paramètres
/// - `layout_tree`  : arbre produit par Taffy (positions brutes)
/// - `styles`       : styles calculés par ferropdf-style
/// - `page_width`   : largeur de la page en points PDF
///
/// # Retour
/// Le même LayoutTree avec les positions Y corrigées (margin collapsing appliqué).
pub fn apply_block_flow(
    layout_tree: &mut LayoutTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    page_width: f32,
) -> Result<(), FerroError> {
    let mut ctx = BlockFormattingContext::new(page_width);

    // Traiter les enfants directs de la racine
    let root_children: Vec<NodeId> = layout_tree.root_children().to_vec();

    for node_id in root_children {
        layout_block_child(node_id, layout_tree, styles, &mut ctx)?;
    }

    Ok(())
}

// =============================================================================
// LAYOUT D'UN ENFANT BLOC
// Inspiré de LayoutBlockFlow::LayoutBlockChild() dans Blink
// =============================================================================

fn layout_block_child(
    node_id: NodeId,
    layout_tree: &mut LayoutTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    ctx: &mut BlockFormattingContext,
) -> Result<(), FerroError> {
    let style = styles
        .get(&node_id)
        .ok_or_else(|| FerroError::Layout(format!("Style manquant pour node {:?}", node_id)))?;

    // Résoudre les marges du bloc courant
    let margins = resolve_margins(style, ctx.containing_width);

    // ─── MARGIN COLLAPSING ───────────────────────────────────────────────────
    // CSS 2.1 §8.3.1 : la marge effective entre deux frères est
    // max(margin_bottom_précédent, margin_top_courant)
    // et NON leur somme.
    let effective_top_margin = collapse_margins(ctx.pending_margin_bottom, margins.top);

    // Positionner le bloc à current_y + marge effective
    let block_y = ctx.current_y + effective_top_margin;

    // Mettre à jour la position Y dans le LayoutTree
    if let Some(layout_box) = layout_tree.get_mut(node_id) {
        layout_box.rect.y = block_y;
    }

    // Récupérer la hauteur du bloc (calculée par Taffy)
    let block_height = layout_tree
        .get(node_id)
        .map(|b| b.rect.height)
        .unwrap_or(0.0);

    // ─── MARGIN COLLAPSING PARENT-ENFANT ────────────────────────────────────
    // CSS 2.1 §8.3.1 cas 2 :
    // Si le parent n'a pas de border-top ni de padding-top,
    // la marge top du parent fusionne avec la marge top du premier enfant.
    if has_no_border_or_padding_top(style) {
        apply_parent_child_margin_collapsing(node_id, layout_tree, styles, block_y)?;
    } else {
        // Le parent a un border/padding → les enfants ont leur propre BFC
        // Récurser normalement
        layout_children_in_new_bfc(node_id, layout_tree, styles, ctx.containing_width)?;
    }

    // Avancer le curseur Y
    ctx.current_y = block_y + block_height;

    // Mémoriser la marge bottom pour le prochain frère
    ctx.pending_margin_bottom = margins.bottom;

    // ─── MARGIN COLLAPSING BLOC VIDE ────────────────────────────────────────
    // CSS 2.1 §8.3.1 cas 4 :
    // Si le bloc est vide (height = 0, pas de padding, pas de border),
    // ses propres marges top et bottom fusionnent.
    if is_empty_block(style, block_height) {
        ctx.pending_margin_bottom = collapse_margins(margins.top, margins.bottom);
        // On ne déplace pas current_y — le bloc vide n'occupe pas d'espace
        ctx.current_y = block_y;
    }

    Ok(())
}

// =============================================================================
// ALGORITHME DE MARGIN COLLAPSING
// CSS 2.1 §8.3.1 — traduit depuis Blink CollapseMargins()
// =============================================================================

/// Fusionne deux marges verticales adjacentes.
/// La règle : la marge effective est le MAX des deux valeurs positives,
/// et la gestion des marges négatives suit la règle CSS 2.1.
///
/// Cas :
///   - Les deux sont positives → max(a, b)
///   - L'une est négative → max_positive + min_negative  (somme algébrique partielle)
///   - Les deux sont négatives → min(a, b)  (la plus négative l'emporte)
fn collapse_margins(margin_a: f32, margin_b: f32) -> f32 {
    match (margin_a >= 0.0, margin_b >= 0.0) {
        // Les deux positives → le max l'emporte
        (true, true) => margin_a.max(margin_b),

        // Les deux négatives → le min l'emporte (la plus négative)
        (false, false) => margin_a.min(margin_b),

        // Une positive, une négative → addition algébrique
        // Exemple : 30px et -10px → 30 + (-10) = 20px
        _ => margin_a + margin_b,
    }
}

/// Fusionne une liste de marges (pour les chaînes de collapse).
/// Utilisé quand plusieurs frères ont height=0 consécutivement.
fn collapse_margin_list(margins: &[f32]) -> f32 {
    let max_positive = margins.iter().cloned().filter(|&m| m >= 0.0).fold(0.0_f32, f32::max);
    let min_negative = margins.iter().cloned().filter(|&m| m < 0.0).fold(0.0_f32, f32::min);

    if min_negative < 0.0 {
        max_positive + min_negative
    } else {
        max_positive
    }
}

// =============================================================================
// MARGIN COLLAPSING PARENT-ENFANT
// CSS 2.1 §8.3.1 cas 2 et 3
// Inspiré de LayoutBlockFlow::HandleAfterSideOfBlock() dans Blink
// =============================================================================

fn apply_parent_child_margin_collapsing(
    parent_id: NodeId,
    layout_tree: &mut LayoutTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    parent_y: f32,
) -> Result<(), FerroError> {
    let children: Vec<NodeId> = layout_tree
        .get(parent_id)
        .map(|b| b.children.clone())
        .unwrap_or_default();

    if children.is_empty() {
        return Ok(());
    }

    // Premier enfant : sa marge top fusionne avec celle du parent
    // → l'enfant est positionné sans marge top supplémentaire
    if let Some(&first_child_id) = children.first() {
        if let Some(first_child_box) = layout_tree.get_mut(first_child_id) {
            // Absorber la marge top de l'enfant dans le parent
            // (le parent "emprunte" la marge de l'enfant)
            first_child_box.rect.y = parent_y;
        }
    }

    // Dernier enfant : sa marge bottom fusionne avec celle du parent
    // Géré lors du calcul de la hauteur du parent dans le post-pass

    Ok(())
}

// =============================================================================
// LAYOUT DES ENFANTS DANS UN NOUVEAU BFC
// Un parent avec border ou padding crée son propre Block Formatting Context
// CSS 2.1 §9.4.1 — les marges ne traversent plus les frontières du parent
// =============================================================================

fn layout_children_in_new_bfc(
    parent_id: NodeId,
    layout_tree: &mut LayoutTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    parent_width: f32,
) -> Result<(), FerroError> {
    let parent_rect = layout_tree
        .get(parent_id)
        .map(|b| b.rect)
        .ok_or_else(|| FerroError::Layout("Parent non trouvé".to_string()))?;

    let children: Vec<NodeId> = layout_tree
        .get(parent_id)
        .map(|b| b.children.clone())
        .unwrap_or_default();

    let mut child_ctx = BlockFormattingContext::new(parent_width);
    // Le curseur Y part du bord intérieur du parent (après padding-top)
    child_ctx.current_y = parent_rect.y;

    for child_id in children {
        layout_block_child(child_id, layout_tree, styles, &mut child_ctx)?;
    }

    Ok(())
}

// =============================================================================
// RÉSOLUTION DES MARGES
// Inspiré de LayoutBox::ComputeAndSetBlockDirectionMargins() dans Blink
// =============================================================================

/// Résout les marges CSS en valeurs absolues (px).
/// Les marges auto sont résolues à 0 en block flow vertical
/// (les marges auto horizontales sont gérées par Taffy).
fn resolve_margins(style: &ComputedStyle, containing_width: f32) -> ResolvedMargins {
    ResolvedMargins {
        top:    resolve_length_to_px(&style.margin_top,    containing_width),
        bottom: resolve_length_to_px(&style.margin_bottom, containing_width),
        left:   resolve_length_to_px(&style.margin_left,   containing_width),
        right:  resolve_length_to_px(&style.margin_right,  containing_width),
    }
}

/// Convertit une valeur Length CSS en pixels absolus.
fn resolve_length_to_px(length: &ferropdf_core::Length, containing_width: f32) -> f32 {
    match length {
        ferropdf_core::Length::Px(px)      => *px,
        ferropdf_core::Length::Percent(p)  => containing_width * p / 100.0,
        ferropdf_core::Length::Em(em)      => em * 16.0, // fallback — doit être résolu avant
        ferropdf_core::Length::Rem(rem)    => rem * 16.0,
        ferropdf_core::Length::Auto        => 0.0,
        ferropdf_core::Length::Zero        => 0.0,
        _ => 0.0,
    }
}

// =============================================================================
// HELPERS
// =============================================================================

/// Vérifie si un bloc a border-top = 0 ET padding-top = 0.
/// Dans ce cas, la marge top du bloc peut fusionner avec celle de son premier enfant.
/// CSS 2.1 §8.3.1 : "if a block has no top border, no top padding"
fn has_no_border_or_padding_top(style: &ComputedStyle) -> bool {
    let border_top  = resolve_length_to_px(&style.border_top_width, 0.0);
    let padding_top = resolve_length_to_px(&style.padding_top, 0.0);
    border_top == 0.0 && padding_top == 0.0
}

/// Vérifie si un bloc est "vide" au sens CSS margin collapsing.
/// CSS 2.1 §8.3.1 cas 4 : bloc vide si height=0, pas de padding, pas de border,
/// pas de clearance, et ne crée pas de nouveau BFC.
fn is_empty_block(style: &ComputedStyle, height: f32) -> bool {
    if height != 0.0 {
        return false;
    }
    let padding_top    = resolve_length_to_px(&style.padding_top,    0.0);
    let padding_bottom = resolve_length_to_px(&style.padding_bottom, 0.0);
    let border_top     = resolve_length_to_px(&style.border_top_width, 0.0);
    let border_bottom  = resolve_length_to_px(&style.border_bottom_width, 0.0);

    padding_top == 0.0 && padding_bottom == 0.0
        && border_top == 0.0 && border_bottom == 0.0
}

// =============================================================================
// POSITIONNEMENT (position: relative, absolute)
// Inspiré de LayoutBox::ApplyRelativePositionIfNeeded() dans Blink
// =============================================================================

/// Applique position: relative sur un LayoutBox.
/// Le bloc reste dans le flux, mais son rendu est décalé visuellement.
/// CSS 2.1 §9.4.3 — Relative positioning
pub fn apply_relative_position(
    layout_box: &mut LayoutBox,
    style: &ComputedStyle,
    containing_width: f32,
    containing_height: f32,
) {
    let offset_left  = resolve_length_to_px(&style.left,  containing_width);
    let offset_right = resolve_length_to_px(&style.right, containing_width);
    let offset_top   = resolve_length_to_px(&style.top,   containing_height);
    let offset_bottom= resolve_length_to_px(&style.bottom,containing_height);

    // Si left et right sont tous deux spécifiés, left l'emporte (LTR)
    // CSS 2.1 §9.4.3
    let dx = if style.left != ferropdf_core::Length::Auto {
        offset_left
    } else if style.right != ferropdf_core::Length::Auto {
        -offset_right
    } else {
        0.0
    };

    let dy = if style.top != ferropdf_core::Length::Auto {
        offset_top
    } else if style.bottom != ferropdf_core::Length::Auto {
        -offset_bottom
    } else {
        0.0
    };

    // Décalage visuel uniquement — ne change pas la position dans le flux
    layout_box.visual_offset_x = dx;
    layout_box.visual_offset_y = dy;
}

/// Applique position: absolute sur un LayoutBox.
/// Le bloc sort du flux et est positionné par rapport au containing block positionné.
/// CSS 2.1 §9.4.2 — Absolute positioning
pub fn apply_absolute_position(
    layout_box: &mut LayoutBox,
    style: &ComputedStyle,
    containing_block_x: f32,
    containing_block_y: f32,
    containing_block_width: f32,
    containing_block_height: f32,
) {
    // Résoudre left/right
    let x = if style.left != ferropdf_core::Length::Auto {
        containing_block_x + resolve_length_to_px(&style.left, containing_block_width)
    } else if style.right != ferropdf_core::Length::Auto {
        containing_block_x
            + containing_block_width
            - layout_box.rect.width
            - resolve_length_to_px(&style.right, containing_block_width)
    } else {
        // Ni left ni right spécifié → position statique d'origine
        layout_box.rect.x
    };

    // Résoudre top/bottom
    let y = if style.top != ferropdf_core::Length::Auto {
        containing_block_y + resolve_length_to_px(&style.top, containing_block_height)
    } else if style.bottom != ferropdf_core::Length::Auto {
        containing_block_y
            + containing_block_height
            - layout_box.rect.height
            - resolve_length_to_px(&style.bottom, containing_block_height)
    } else {
        layout_box.rect.y
    };

    layout_box.rect.x = x;
    layout_box.rect.y = y;
    // Marquer comme hors flux (le renderer PDF doit le dessiner en dernier)
    layout_box.out_of_flow = true;
}

// =============================================================================
// TESTS UNITAIRES
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ─── Tests margin collapsing ────────────────────────────────────────────

    #[test]
    fn test_collapse_deux_positifs() {
        // 30px et 20px → 30px (le max l'emporte)
        assert_eq!(collapse_margins(30.0, 20.0), 30.0);
    }

    #[test]
    fn test_collapse_deux_negatifs() {
        // -10px et -20px → -20px (le plus négatif l'emporte)
        assert_eq!(collapse_margins(-10.0, -20.0), -20.0);
    }

    #[test]
    fn test_collapse_positif_negatif() {
        // 30px et -10px → 20px (addition algébrique)
        assert_eq!(collapse_margins(30.0, -10.0), 20.0);
    }

    #[test]
    fn test_collapse_symetrique() {
        // 48px et 48px → 48px (même valeur)
        assert_eq!(collapse_margins(48.0, 48.0), 48.0);
    }

    #[test]
    fn test_collapse_zero() {
        // 0px et 20px → 20px
        assert_eq!(collapse_margins(0.0, 20.0), 20.0);
    }

    #[test]
    fn test_collapse_margin_list() {
        // [10, 20, -5] → max_positive=20, min_negative=-5, résultat=15
        assert_eq!(collapse_margin_list(&[10.0, 20.0, -5.0]), 15.0);
    }

    #[test]
    fn test_collapse_margin_list_tous_positifs() {
        // [10, 30, 20] → max=30
        assert_eq!(collapse_margin_list(&[10.0, 30.0, 20.0]), 30.0);
    }

    // ─── Tests resolve_length ───────────────────────────────────────────────

    #[test]
    fn test_resolve_px() {
        use ferropdf_core::Length;
        assert_eq!(resolve_length_to_px(&Length::Px(16.0), 800.0), 16.0);
    }

    #[test]
    fn test_resolve_percent() {
        use ferropdf_core::Length;
        // 50% de 800px → 400px
        assert_eq!(resolve_length_to_px(&Length::Percent(50.0), 800.0), 400.0);
    }

    #[test]
    fn test_resolve_auto_est_zero() {
        use ferropdf_core::Length;
        assert_eq!(resolve_length_to_px(&Length::Auto, 800.0), 0.0);
    }

    // ─── Tests bloc vide ────────────────────────────────────────────────────

    #[test]
    fn test_is_empty_block_vrai() {
        let style = ComputedStyle {
            padding_top: ferropdf_core::Length::Zero,
            padding_bottom: ferropdf_core::Length::Zero,
            border_top_width: ferropdf_core::Length::Zero,
            border_bottom_width: ferropdf_core::Length::Zero,
            ..Default::default()
        };
        assert!(is_empty_block(&style, 0.0));
    }

    #[test]
    fn test_is_empty_block_faux_hauteur() {
        let style = ComputedStyle::default();
        // Hauteur non nulle → pas un bloc vide
        assert!(!is_empty_block(&style, 50.0));
    }

    #[test]
    fn test_is_empty_block_faux_padding() {
        let style = ComputedStyle {
            padding_top: ferropdf_core::Length::Px(10.0),
            ..Default::default()
        };
        assert!(!is_empty_block(&style, 0.0));
    }
}
