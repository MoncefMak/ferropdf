// =============================================================================
// block_flow.rs — Algorithme de layout Block Flow (CSS 2.1 §8.3.1)
// =============================================================================
// Traduit depuis les algorithmes de Blink (BSD licence) :
//   blink/renderer/core/layout/layout_block_flow.cc
//   blink/renderer/core/layout/layout_block.cc
//
// Ce module s'exécute APRÈS Taffy comme un post-pass.
// Taffy calcule les dimensions (width, height, padding, border).
// Ce module corrige les positions Y pour le margin collapsing
// et applique position: relative/absolute.
// =============================================================================

use ferropdf_core::{LayoutBox, LayoutTree, ComputedStyle, Position, Length, Display as FDisplay};

// =============================================================================
// STRUCTURES DE DONNÉES
// =============================================================================

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
pub fn apply_block_flow(layout_tree: &mut LayoutTree, page_width: f32) {
    if let Some(ref mut root) = layout_tree.root {
        let mut ctx = BlockFormattingContext::new(page_width);
        layout_block_children(root, &mut ctx);
    }
}

// =============================================================================
// LAYOUT DES ENFANTS BLOC — VERSION RÉCURSIVE
// Inspiré de LayoutBlockFlow::LayoutBlockChild() dans Blink
// =============================================================================

/// Applique le margin collapsing + positionnement sur les enfants d'un layout_box.
fn layout_block_children(parent: &mut LayoutBox, ctx: &mut BlockFormattingContext) {
    // Block flow only applies to block-level containers.
    // Skip tables (grid), flex, inline — their children are positioned by Taffy.
    match parent.style.display {
        FDisplay::Table | FDisplay::TableRow | FDisplay::TableCell
        | FDisplay::TableHeaderGroup | FDisplay::TableRowGroup | FDisplay::TableFooterGroup
        | FDisplay::Flex | FDisplay::Grid | FDisplay::Inline | FDisplay::InlineBlock => {
            return;
        }
        _ => {}
    }

    let parent_y = parent.rect.y;
    let parent_height = parent.rect.height;
    let parent_width = ctx.containing_width;

    // Si le parent a border-top ou padding-top, il crée un nouveau BFC
    // → les marges des enfants ne fusionnent pas avec l'extérieur
    let creates_new_bfc = parent.border.top > 0.0
        || resolve_length_to_px(&parent.style.padding[0], parent_width) > 0.0;

    let mut child_ctx = if creates_new_bfc {
        // Nouveau BFC: curseur part du bord intérieur du parent (après padding-top)
        let mut bfc = BlockFormattingContext::new(parent_width);
        bfc.current_y = parent_y + parent.border.top + parent.padding.top;
        bfc
    } else {
        // Pas de BFC propre: utiliser le contexte du parent
        let mut bfc = BlockFormattingContext::new(parent_width);
        bfc.current_y = parent_y;
        bfc.pending_margin_bottom = ctx.pending_margin_bottom;
        bfc
    };

    for child in &mut parent.children {
        if child.out_of_flow {
            continue;
        }

        // Skip zero-height whitespace text nodes — they don't participate in block flow
        // and would incorrectly reset pending_margin_bottom (CSS 2.1 §9.2.1.1)
        if child.rect.height < 0.5 && child.text_content.is_some() {
            let is_ws = child.text_content.as_ref()
                .map(|t| t.trim().is_empty())
                .unwrap_or(false);
            if is_ws {
                continue;
            }
        }

        // Extract style values before mutating child (borrow checker)
        let margin_top = resolve_length_to_px(&child.style.margin[0], child_ctx.containing_width);
        let margin_bottom = resolve_length_to_px(&child.style.margin[2], child_ctx.containing_width);
        let is_relative = child.style.position == Position::Relative;
        let block_height = child.rect.height;
        let is_empty = is_empty_block(&child.style, block_height);

        // ─── MARGIN COLLAPSING ──────────────────────────────────────────
        // CSS 2.1 §8.3.1 : la marge effective entre deux frères est
        // max(margin_bottom_précédent, margin_top_courant)
        // et NON leur somme.
        let effective_top_margin = collapse_margins(child_ctx.pending_margin_bottom, margin_top);

        // Positionner le bloc à current_y + marge effective
        let block_y = child_ctx.current_y + effective_top_margin;

        // Mettre à jour la position Y du LayoutBox
        let dy = block_y - child.rect.y;
        child.rect.y = block_y;
        child.content.y += dy;

        // Propagate Y shift to all descendants.
        // Children have absolute coordinates from read_layout, based on the old
        // parent position. We must shift them by the same dy so they stay
        // correctly positioned relative to the parent.
        if dy.abs() > 0.001 {
            shift_subtree_y(&mut child.children, dy);
        }

        // Récurser dans les enfants — for block containers, this recalculates
        // children positions (overriding the shift above when appropriate).
        layout_block_children(child, &mut child_ctx);

        // ─── POSITION: RELATIVE ──────────────────────────────────────────
        if is_relative {
            apply_relative_position(child, parent_width, parent_height);
        }

        // Avancer le curseur Y
        child_ctx.current_y = block_y + block_height;

        // Mémoriser la marge bottom pour le prochain frère
        child_ctx.pending_margin_bottom = margin_bottom;

        // ─── MARGIN COLLAPSING BLOC VIDE ────────────────────────────────
        // CSS 2.1 §8.3.1 cas 4
        if is_empty {
            child_ctx.pending_margin_bottom = collapse_margins(margin_top, margin_bottom);
            child_ctx.current_y = block_y;
        }
    }

    // Propagate pending_margin_bottom back to parent context if no BFC boundary
    if !creates_new_bfc {
        ctx.pending_margin_bottom = child_ctx.pending_margin_bottom;
    }
}

/// Recursively shift all descendants' Y positions by `dy`.
/// Used when block_flow repositions a parent — children with absolute positions
/// (computed from read_layout using the old parent position) must be updated.
fn shift_subtree_y(children: &mut [LayoutBox], dy: f32) {
    for child in children.iter_mut() {
        child.rect.y += dy;
        child.content.y += dy;
        shift_subtree_y(&mut child.children, dy);
    }
}

// =============================================================================
// ALGORITHME DE MARGIN COLLAPSING
// CSS 2.1 §8.3.1 — traduit depuis Blink CollapseMargins()
// =============================================================================

/// Fusionne deux marges verticales adjacentes.
/// - Les deux positives → max(a, b)
/// - Les deux négatives → min(a, b)
/// - Une positive, une négative → somme algébrique
fn collapse_margins(margin_a: f32, margin_b: f32) -> f32 {
    match (margin_a >= 0.0, margin_b >= 0.0) {
        (true, true) => margin_a.max(margin_b),
        (false, false) => margin_a.min(margin_b),
        _ => margin_a + margin_b,
    }
}

// =============================================================================
// POSITIONNEMENT (position: relative)
// Inspiré de LayoutBox::ApplyRelativePositionIfNeeded() dans Blink
// CSS 2.1 §9.4.3
// =============================================================================

fn apply_relative_position(
    layout_box: &mut LayoutBox,
    containing_width: f32,
    containing_height: f32,
) {
    let style = &layout_box.style;

    let offset_left  = resolve_length_to_px(&style.left,  containing_width);
    let offset_right = resolve_length_to_px(&style.right, containing_width);
    let offset_top   = resolve_length_to_px(&style.top,   containing_height);
    let offset_bottom= resolve_length_to_px(&style.bottom, containing_height);

    // Si left et right sont tous deux spécifiés, left l'emporte (LTR)
    let dx = if style.left != Length::Auto {
        offset_left
    } else if style.right != Length::Auto {
        -offset_right
    } else {
        0.0
    };

    let dy = if style.top != Length::Auto {
        offset_top
    } else if style.bottom != Length::Auto {
        -offset_bottom
    } else {
        0.0
    };

    layout_box.visual_offset_x = dx;
    layout_box.visual_offset_y = dy;
}

// =============================================================================
// HELPERS
// =============================================================================

/// Vérifie si un bloc est "vide" au sens CSS margin collapsing.
/// CSS 2.1 §8.3.1 cas 4
fn is_empty_block(style: &ComputedStyle, height: f32) -> bool {
    if height != 0.0 {
        return false;
    }
    let padding_top    = resolve_length_to_px(&style.padding[0], 0.0);
    let padding_bottom = resolve_length_to_px(&style.padding[2], 0.0);

    padding_top == 0.0 && padding_bottom == 0.0
        && style.border_top.width == 0.0 && style.border_bottom.width == 0.0
}

/// Convertit une valeur Length CSS en pixels absolus.
fn resolve_length_to_px(length: &Length, containing_width: f32) -> f32 {
    match length {
        Length::Px(px)      => *px,
        Length::Percent(p)  => containing_width * p / 100.0,
        Length::Em(em)      => em * 16.0,
        Length::Rem(rem)    => rem * 16.0,
        Length::Auto        => 0.0,
        Length::Zero        => 0.0,
        _                   => 0.0,
    }
}

// =============================================================================
// TESTS UNITAIRES
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_deux_positifs() {
        assert_eq!(collapse_margins(30.0, 20.0), 30.0);
    }

    #[test]
    fn test_collapse_deux_negatifs() {
        assert_eq!(collapse_margins(-10.0, -20.0), -20.0);
    }

    #[test]
    fn test_collapse_positif_negatif() {
        assert_eq!(collapse_margins(30.0, -10.0), 20.0);
    }

    #[test]
    fn test_collapse_symetrique() {
        assert_eq!(collapse_margins(48.0, 48.0), 48.0);
    }

    #[test]
    fn test_collapse_zero() {
        assert_eq!(collapse_margins(0.0, 20.0), 20.0);
    }
}
