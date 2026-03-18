// =============================================================================
// pagination.rs — Algorithme de fragmentation et pagination PDF
// =============================================================================
// Source d'inspiration :
//   CSS Fragmentation Module Level 3
//   https://www.w3.org/TR/css-break-3/
//
//   Blink fragmentation utils :
//   blink/renderer/core/layout/fragmentation_utils.cc
//   blink/renderer/core/layout/ng/ng_block_break_token.cc
//
// Ce module s'exécute APRÈS Taffy + block_flow.
// Il prend le LayoutTree (ruban infini de contenu) et le découpe en pages.
//
// MODÈLE MENTAL :
//   Le ruban de contenu infini (positions Y de 0 à +infini) est découpé
//   en tranches de hauteur = page_height.
//   Les coordonnées Y de chaque LayoutBox sont recalculées en soustrayant
//   l'offset de la page : y_pdf = y_original - (page_idx * page_height)
// =============================================================================

use ferropdf_core::{LayoutBox, PageConfig, PageBreak, PageBreakInside};
use ferropdf_core::layout::Page;

// =============================================================================
// STRUCTURES DE DONNÉES
// =============================================================================

/// État courant du paginateur — inspiré de FragmentainerContext dans Blink.
#[derive(Debug)]
struct PaginationContext {
    /// Position Y du curseur dans le ruban de contenu
    current_y: f32,
    /// Contenu de la page en cours de construction
    current_page_boxes: Vec<LayoutBox>,
    /// Pages déjà terminées
    finished_pages: Vec<Page>,
    /// Hauteur d'une page (content area)
    page_height: f32,
}

impl PaginationContext {
    fn new(page_height: f32) -> Self {
        Self {
            current_y: 0.0,
            current_page_boxes: Vec::new(),
            finished_pages: Vec::new(),
            page_height,
        }
    }

    /// Flush la page courante et en commence une nouvelle.
    fn flush_page(&mut self) {
        if !self.current_page_boxes.is_empty() {
            let page_number = self.finished_pages.len() as u32 + 1;
            self.finished_pages.push(Page {
                page_number,
                total_pages: 0,
                content: std::mem::take(&mut self.current_page_boxes),
                margin_boxes: Vec::new(),
            });
            self.current_y = 0.0;
        }
    }

    fn is_current_page_empty(&self) -> bool {
        self.current_page_boxes.is_empty()
    }
}

// =============================================================================
// POINT D'ENTRÉE PRINCIPAL
// =============================================================================

/// Fragmente un LayoutTree root en pages PDF.
pub fn paginate(root: &LayoutBox, config: &PageConfig) -> Vec<Page> {
    let page_height = config.content_height_px();
    let mut ctx = PaginationContext::new(page_height);

    // Traiter les enfants directs de la racine
    for child in &root.children {
        fragment_box(child, &mut ctx);
    }

    // Flush la dernière page si non vide
    if !ctx.is_current_page_empty() {
        ctx.flush_page();
    }

    // Si aucune page n'a été produite, créer une page vide
    if ctx.finished_pages.is_empty() {
        ctx.finished_pages.push(Page {
            page_number: 1,
            total_pages: 1,
            content: Vec::new(),
            margin_boxes: Vec::new(),
        });
    }

    // Update total_pages
    let total = ctx.finished_pages.len() as u32;
    for page in &mut ctx.finished_pages {
        page.total_pages = total;
    }

    ctx.finished_pages
}

// =============================================================================
// FRAGMENTATION D'UN LAYOUT BOX
// CSS Fragmentation Level 3 §4 — Fragmentation Model
// =============================================================================

fn fragment_box(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    let style = &layout_box.style;
    let box_height = layout_box.rect.height;

    // ─── Règle 1 : page-break-before ────────────────────────────────────────
    if should_break_before(style) && !ctx.is_current_page_empty() {
        ctx.flush_page();
    }

    // ─── Règle 2 : page-break-inside: avoid ─────────────────────────────────
    let avoid_break_inside = style.page_break_inside == PageBreakInside::Avoid;
    let fits_on_current_page = ctx.current_y + box_height <= ctx.page_height;
    let fits_on_new_page = box_height <= ctx.page_height;

    if !fits_on_current_page && avoid_break_inside {
        if fits_on_new_page {
            if !ctx.is_current_page_empty() {
                ctx.flush_page();
            }
            place_box_on_current_page(layout_box, ctx);
            if should_break_after(style) {
                ctx.flush_page();
            }
            return;
        }
        // Le bloc est plus grand qu'une page → on ne peut pas éviter la coupure
    }

    // ─── Règle 3 : Le bloc tient sur la page courante ───────────────────────
    if fits_on_current_page {
        place_box_on_current_page(layout_box, ctx);
        if should_break_after(style) {
            ctx.flush_page();
        }
        return;
    }

    // ─── Règle 4 : Le bloc ne tient pas → fragmentation ─────────────────────
    if !layout_box.children.is_empty() {
        for child in &layout_box.children {
            fragment_box(child, ctx);
        }
    } else {
        // Boîte feuille — protection anti-boucle infinie
        if ctx.is_current_page_empty() {
            place_box_on_current_page(layout_box, ctx);
            ctx.flush_page();
        } else {
            ctx.flush_page();
            place_box_on_current_page(layout_box, ctx);
            if box_height > ctx.page_height {
                ctx.flush_page();
            }
        }
    }

    // ─── Règle 5 : page-break-after ─────────────────────────────────────────
    if should_break_after(style) && !ctx.is_current_page_empty() {
        ctx.flush_page();
    }
}

// =============================================================================
// PLACEMENT D'UNE BOX SUR LA PAGE COURANTE
// Avec repositionnement Y (Y absolu → Y relatif à la page)
// =============================================================================

fn place_box_on_current_page(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    let page_idx = ctx.finished_pages.len();
    let y_offset = page_idx as f32 * ctx.page_height;

    // Repositionner Y : y_pdf = y_original - offset_page
    let mut placed_box = layout_box.clone();
    placed_box.rect.y = (layout_box.rect.y - y_offset).max(0.0);
    placed_box.content.y = (layout_box.content.y - y_offset).max(0.0);

    ctx.current_y += placed_box.rect.height;
    ctx.current_page_boxes.push(placed_box);
}

// =============================================================================
// HELPERS — Détection des règles CSS de fragmentation
// =============================================================================

fn should_break_before(style: &ferropdf_core::ComputedStyle) -> bool {
    matches!(
        style.page_break_before,
        PageBreak::Always | PageBreak::Page | PageBreak::Left | PageBreak::Right
    )
}

fn should_break_after(style: &ferropdf_core::ComputedStyle) -> bool {
    matches!(
        style.page_break_after,
        PageBreak::Always | PageBreak::Page | PageBreak::Left | PageBreak::Right
    )
}

/// Crée une page vide.
pub fn create_empty_page(config: &PageConfig) -> Page {
    Page {
        page_number: 1,
        total_pages: 1,
        content: Vec::new(),
        margin_boxes: Vec::new(),
    }
}
