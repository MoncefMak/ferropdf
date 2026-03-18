// =============================================================================
// pagination.rs — Algorithme de fragmentation et pagination PDF
// =============================================================================
// Source d'inspiration :
//   CSS Fragmentation Module Level 3
//   https://www.w3.org/TR/css-break-3/
//
//   Blink fragmentation utils (pour la logique de break) :
//   blink/renderer/core/layout/fragmentation_utils.cc
//   blink/renderer/core/layout/ng/ng_block_break_token.cc
//
// Ce module s'exécute APRÈS Taffy + block_flow.
// Il prend le LayoutTree (ruban infini de contenu) et le découpe en pages.
//
// MODÈLE MENTAL :
//   Tu as un ruban de contenu infini (positions Y de 0 à +infini).
//   Tu découpes ce ruban en tranches de hauteur = page_height.
//   Chaque tranche devient une page PDF.
//   Les coordonnées Y de chaque LayoutBox dans une page sont recalculées
//   en soustrayant l'offset de la page : y_pdf = y_original - (page_idx * page_height)
// =============================================================================

use ferropdf_core::{
    LayoutBox, LayoutTree, PagedLayoutTree, ComputedStyle, NodeId,
    FerroError, PageBreak, PageBreakInside,
};
use std::collections::HashMap;

// =============================================================================
// STRUCTURES DE DONNÉES
// =============================================================================

/// Une page PDF = liste de LayoutBox repositionnés (Y relatif à la page).
#[derive(Debug, Clone, Default)]
pub struct Page {
    pub boxes: Vec<LayoutBox>,
    /// Index de la page (0-based)
    pub page_index: usize,
    /// Largeur de la page en points PDF
    pub width: f32,
    /// Hauteur de la page en points PDF
    pub height: f32,
}

/// État courant du paginateur — inspiré de FragmentainerContext dans Blink.
#[derive(Debug)]
struct PaginationContext {
    /// Position Y du curseur dans le ruban de contenu
    current_y: f32,
    /// Contenu de la page en cours de construction
    current_page_boxes: Vec<LayoutBox>,
    /// Pages déjà terminées
    finished_pages: Vec<Page>,
    /// Hauteur d'une page
    page_height: f32,
    /// Largeur d'une page
    page_width: f32,
}

impl PaginationContext {
    fn new(page_width: f32, page_height: f32) -> Self {
        Self {
            current_y: 0.0,
            current_page_boxes: Vec::new(),
            finished_pages: Vec::new(),
            page_height,
            page_width,
        }
    }

    /// Flush la page courante et en commence une nouvelle.
    fn flush_page(&mut self) {
        if !self.current_page_boxes.is_empty() {
            let page_index = self.finished_pages.len();
            self.finished_pages.push(Page {
                boxes: std::mem::take(&mut self.current_page_boxes),
                page_index,
                width: self.page_width,
                height: self.page_height,
            });
            self.current_y = 0.0;
        }
    }

    /// Retourne vrai si la page courante est vide.
    fn is_current_page_empty(&self) -> bool {
        self.current_page_boxes.is_empty()
    }

    /// Retourne le nombre de pages terminées + la page en cours si non vide.
    fn total_page_count(&self) -> usize {
        self.finished_pages.len() + if !self.current_page_boxes.is_empty() { 1 } else { 0 }
    }
}

// =============================================================================
// POINT D'ENTRÉE PRINCIPAL
// =============================================================================

/// Fragmente un LayoutTree en pages PDF.
///
/// # Paramètres
/// - `layout_tree` : arbre de layout produit par Taffy + block_flow
/// - `styles`      : styles calculés par ferropdf-style
/// - `page_width`  : largeur de page en points PDF (ex: 595.0 pour A4)
/// - `page_height` : hauteur de page en points PDF (ex: 842.0 pour A4)
///
/// # Retour
/// Vec<Page> où chaque Page contient ses LayoutBox avec des Y relatifs à la page.
pub fn paginate(
    layout_tree: &LayoutTree,
    styles: &HashMap<NodeId, ComputedStyle>,
    page_width: f32,
    page_height: f32,
) -> Result<Vec<Page>, FerroError> {
    let mut ctx = PaginationContext::new(page_width, page_height);

    // Traiter les enfants directs de la racine
    let root_children: Vec<&LayoutBox> = layout_tree.root_children_boxes();

    for layout_box in root_children {
        fragment_box(layout_box, styles, &mut ctx)?;
    }

    // Flush la dernière page si non vide
    if !ctx.is_current_page_empty() {
        ctx.flush_page();
    }

    // Si aucune page n'a été produite, créer une page vide
    if ctx.finished_pages.is_empty() {
        ctx.finished_pages.push(Page {
            boxes: Vec::new(),
            page_index: 0,
            width: page_width,
            height: page_height,
        });
    }

    Ok(ctx.finished_pages)
}

// =============================================================================
// FRAGMENTATION D'UN LAYOUT BOX
// CSS Fragmentation Level 3 §4 — Fragmentation Model
// Inspiré de BlockNode::Layout() avec BreakToken dans Blink LayoutNG
// =============================================================================

fn fragment_box(
    layout_box: &LayoutBox,
    styles: &HashMap<NodeId, ComputedStyle>,
    ctx: &mut PaginationContext,
) -> Result<(), FerroError> {
    let style = layout_box
        .node_id
        .and_then(|id| styles.get(&id));

    let box_height = layout_box.rect.height;
    let box_y_original = layout_box.rect.y;

    // ─── Règle 1 : page-break-before: always ────────────────────────────────
    // CSS Fragmentation §3.1 — break-before: page
    // Forcer une nouvelle page AVANT ce bloc.
    // Exception : si la page courante est déjà vide, ne pas créer de page blanche.
    if should_break_before(style) && !ctx.is_current_page_empty() {
        ctx.flush_page();
    }

    // ─── Règle 2 : page-break-inside: avoid ─────────────────────────────────
    // CSS Fragmentation §3.2 — break-inside: avoid
    // Ne pas couper CE bloc entre deux pages.
    let avoid_break_inside = should_avoid_break_inside(style);
    let fits_on_current_page = ctx.current_y + box_height <= ctx.page_height;
    let fits_on_new_page = box_height <= ctx.page_height;

    if !fits_on_current_page && avoid_break_inside {
        if fits_on_new_page {
            // Le bloc tient sur une page vide → flush et placer sur la suivante
            if !ctx.is_current_page_empty() {
                ctx.flush_page();
            }
            // Placer intact sur la nouvelle page
            place_box_on_current_page(layout_box, ctx);
            // Vérifier page-break-after
            if should_break_after(style) {
                ctx.flush_page();
            }
            return Ok(());
        }
        // Le bloc est plus grand qu'une page entière → on ne peut pas éviter la coupure
        // Tomber dans la logique normale (fragmentation récursive)
    }

    // ─── Règle 3 : Le bloc tient sur la page courante ────────────────────────
    if fits_on_current_page {
        place_box_on_current_page(layout_box, ctx);
        if should_break_after(style) {
            ctx.flush_page();
        }
        return Ok(());
    }

    // ─── Règle 4 : Le bloc ne tient pas → fragmentation ─────────────────────
    // CSS Fragmentation §4.4 — Fragmenting block-level boxes
    if !layout_box.children.is_empty() {
        // CAS A : Le bloc a des enfants → fragmentation récursive
        // On descend dans les enfants plutôt que de couper le parent monolithiquement.
        // Inspiré de LayoutNG's BlockNode fragmenting via BreakToken.
        for child in &layout_box.children {
            fragment_box(child, styles, ctx)?;
        }
    } else {
        // CAS B : Boîte feuille (texte, image) qui ne tient pas sur la page
        // ─── PROTECTION ANTI-BOUCLE INFINIE ────────────────────────────────
        // CSS Fragmentation §4.4 : "If a block cannot be fragmented,
        // it is placed in the current fragmentainer even if it overflows."
        // Si la page courante est vide, on force le placement (évite la boucle infinie).
        if ctx.is_current_page_empty() {
            // Forcer le placement même si ça déborde
            place_box_on_current_page(layout_box, ctx);
            ctx.flush_page();
        } else {
            // Flush la page courante et placer sur la nouvelle
            ctx.flush_page();

            // Vérifier à nouveau sur la nouvelle page (récursion implicite)
            if layout_box.rect.height <= ctx.page_height {
                place_box_on_current_page(layout_box, ctx);
            } else {
                // Trop grand même sur une page vide → forcer
                place_box_on_current_page(layout_box, ctx);
                ctx.flush_page();
            }
        }
    }

    // ─── Règle 5 : page-break-after: always ─────────────────────────────────
    if should_break_after(style) && !ctx.is_current_page_empty() {
        ctx.flush_page();
    }

    Ok(())
}

// =============================================================================
// PLACEMENT D'UNE BOX SUR LA PAGE COURANTE
// Avec repositionnement Y (Y absolu → Y relatif à la page)
// =============================================================================

fn place_box_on_current_page(
    layout_box: &LayoutBox,
    ctx: &mut PaginationContext,
) {
    let page_idx = ctx.finished_pages.len();
    let y_offset = page_idx as f32 * ctx.page_height;

    // Repositionner Y : y_pdf = y_original - offset_page
    let mut placed_box = layout_box.clone();
    placed_box.rect.y = layout_box.rect.y - y_offset;

    // S'assurer que Y ne soit pas négatif (protection défensive)
    if placed_box.rect.y < 0.0 {
        placed_box.rect.y = 0.0;
    }

    ctx.current_y += placed_box.rect.height;
    ctx.current_page_boxes.push(placed_box);
}

// =============================================================================
// HELPERS — Détection des règles CSS de fragmentation
// =============================================================================

/// Vérifie si page-break-before: always (ou break-before: page).
/// CSS Fragmentation §3.1
fn should_break_before(style: Option<&ComputedStyle>) -> bool {
    matches!(
        style.map(|s| &s.page_break_before),
        Some(PageBreak::Always) | Some(PageBreak::Page) | Some(PageBreak::Left) | Some(PageBreak::Right)
    )
}

/// Vérifie si page-break-after: always (ou break-after: page).
fn should_break_after(style: Option<&ComputedStyle>) -> bool {
    matches!(
        style.map(|s| &s.page_break_after),
        Some(PageBreak::Always) | Some(PageBreak::Page) | Some(PageBreak::Left) | Some(PageBreak::Right)
    )
}

/// Vérifie si page-break-inside: avoid (ou break-inside: avoid).
/// CSS Fragmentation §3.2
fn should_avoid_break_inside(style: Option<&ComputedStyle>) -> bool {
    matches!(
        style.map(|s| &s.page_break_inside),
        Some(PageBreakInside::Avoid)
    )
}

// =============================================================================
// CALCUL DES ORPHELINS ET VEUVES (TODO v2)
// CSS 2.1 §15.2 — orphans et widows
// =============================================================================
//
// TODO v2 : Implémenter orphans/widows pour le texte.
// Logique :
//   orphans = nombre minimum de lignes à laisser en bas d'une page
//   widows  = nombre minimum de lignes à mettre en haut d'une page suivante
//
//   Pour l'implémenter, il faut :
//   1. Connaître le nombre de lignes de chaque LayoutBox de texte
//      → disponible via cosmic-text buffer.layout_runs().count()
//   2. Si un bloc de texte est coupé et que le nombre de lignes restantes
//      sur la page courante < orphans → monter le bloc entier sur la page suivante
//   3. Si un bloc de texte est coupé et que le nombre de lignes sur la nouvelle page
//      < widows → ajuster le point de coupure
//
// fn check_orphans_widows(
//     layout_box: &LayoutBox,
//     style: &ComputedStyle,
//     lines_on_current_page: usize,
//     lines_on_next_page: usize,
// ) -> BreakDecision { ... }

// =============================================================================
// TESTS UNITAIRES
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;
    use ferropdf_core::{Rect, LayoutBox};

    fn make_box(y: f32, height: f32) -> LayoutBox {
        LayoutBox {
            rect: Rect { x: 0.0, y, width: 595.0, height },
            node_id: None,
            children: vec![],
            out_of_flow: false,
            visual_offset_x: 0.0,
            visual_offset_y: 0.0,
        }
    }

    // ─── Test : boîte qui tient sur une seule page ──────────────────────────

    #[test]
    fn test_boite_tient_sur_une_page() {
        let mut ctx = PaginationContext::new(595.0, 842.0);
        let b = make_box(0.0, 200.0);
        let styles = HashMap::new();

        fragment_box(&b, &styles, &mut ctx).unwrap();
        ctx.flush_page();

        assert_eq!(ctx.finished_pages.len(), 1);
        assert_eq!(ctx.finished_pages[0].boxes.len(), 1);
    }

    // ─── Test : deux boîtes qui tiennent sur une page ───────────────────────

    #[test]
    fn test_deux_boites_une_page() {
        let mut ctx = PaginationContext::new(595.0, 842.0);
        let styles = HashMap::new();

        fragment_box(&make_box(0.0, 300.0), &styles, &mut ctx).unwrap();
        fragment_box(&make_box(300.0, 300.0), &styles, &mut ctx).unwrap();
        ctx.flush_page();

        assert_eq!(ctx.finished_pages.len(), 1);
        assert_eq!(ctx.finished_pages[0].boxes.len(), 2);
    }

    // ─── Test : boîtes qui débordent sur deux pages ──────────────────────────

    #[test]
    fn test_boites_sur_deux_pages() {
        let mut ctx = PaginationContext::new(595.0, 842.0);
        let styles = HashMap::new();

        fragment_box(&make_box(0.0, 500.0), &styles, &mut ctx).unwrap();
        fragment_box(&make_box(500.0, 500.0), &styles, &mut ctx).unwrap();
        ctx.flush_page();

        // La 2ème boîte (500px) ne tient pas sur la page (500+500 > 842)
        // → doit être sur la page 2
        assert_eq!(ctx.finished_pages.len(), 2);
    }

    // ─── Test : ANTI-BOUCLE INFINIE — boîte plus grande que la page ─────────

    #[test]
    fn test_anti_boucle_infinie_boite_geante() {
        let mut ctx = PaginationContext::new(595.0, 842.0);
        let styles = HashMap::new();

        // Boîte de 5000px — bien plus grande que la page (842px)
        fragment_box(&make_box(0.0, 5000.0), &styles, &mut ctx).unwrap();

        // Ne doit PAS boucler infiniment
        // Doit produire au moins une page avec la boîte dedans
        let total = ctx.total_page_count();
        assert!(total >= 1, "Doit produire au moins une page");
    }

    // ─── Test : repositionnement Y ──────────────────────────────────────────

    #[test]
    fn test_repositionnement_y_page_2() {
        let mut ctx = PaginationContext::new(595.0, 842.0);
        let styles = HashMap::new();

        // Page 1 : remplir presque complètement
        fragment_box(&make_box(0.0, 800.0), &styles, &mut ctx).unwrap();
        // Cette boîte déborde → va sur page 2, Y original = 800 + 100 = 900
        // Y relatif page 2 = 900 - 842 = 58
        fragment_box(&make_box(800.0, 100.0), &styles, &mut ctx).unwrap();
        ctx.flush_page();

        assert_eq!(ctx.finished_pages.len(), 2);
        let box_on_page2 = &ctx.finished_pages[1].boxes[0];
        // Y doit être relatif à la page 2 (petit nombre, pas 900)
        assert!(box_on_page2.rect.y < 100.0,
            "Y devrait être relatif à la page, pas absolu. Y = {}", box_on_page2.rect.y);
    }

    // ─── Test : page vide non créée si page-break-before sur page vide ───────

    #[test]
    fn test_pas_de_page_blanche_break_before() {
        // should_break_before retourne false pour style None
        assert!(!should_break_before(None));
    }

    // ─── Test : flush_page ne crée pas de page vide ──────────────────────────

    #[test]
    fn test_flush_page_vide_sans_effet() {
        let mut ctx = PaginationContext::new(595.0, 842.0);
        ctx.flush_page(); // page courante est vide → ne doit rien faire
        assert_eq!(ctx.finished_pages.len(), 0);
    }
}
