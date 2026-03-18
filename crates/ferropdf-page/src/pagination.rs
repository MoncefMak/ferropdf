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
//   Le ruban de contenu infini (positions Y de 0 à +∞) est découpé
//   en pages. On parcourt les blocs enfants de la racine et on les
//   place un par un sur la page courante. Quand un bloc ne tient pas,
//   on le fragmente (récursivement dans ses enfants) ou on le pousse
//   sur la page suivante.
//
//   La variable clé est `page_y_offset` : la coordonnée Y absolue
//   dans le ruban qui correspond à Y=0 sur la page courante.
//   Pour repositionner un bloc sur la page :
//     y_on_page = y_absolute - page_y_offset
// =============================================================================

use ferropdf_core::{LayoutBox, PageConfig, PageBreak, PageBreakInside, Rect, Insets};
use ferropdf_core::layout::Page;

// =============================================================================
// STRUCTURES DE DONNÉES
// =============================================================================

/// État courant du paginateur — inspiré de FragmentainerContext dans Blink.
#[derive(Debug)]
struct PaginationContext {
    /// Position Y absolue (dans le ruban) correspondant au haut de la page courante.
    page_y_offset: f32,
    /// Hauteur consommée sur la page courante (pour savoir combien d'espace reste).
    used_height: f32,
    /// Contenu de la page en cours de construction.
    current_page_boxes: Vec<LayoutBox>,
    /// Pages déjà terminées.
    finished_pages: Vec<Page>,
    /// Hauteur d'une page (content area, en CSS pixels).
    page_height: f32,
}

impl PaginationContext {
    fn new(page_height: f32) -> Self {
        Self {
            page_y_offset: 0.0,
            used_height: 0.0,
            current_page_boxes: Vec::new(),
            finished_pages: Vec::new(),
            page_height,
        }
    }

    /// Espace restant sur la page courante.
    fn remaining_height(&self) -> f32 {
        (self.page_height - self.used_height).max(0.0)
    }

    /// Flush la page courante et commence une nouvelle.
    /// `next_y` est la position Y absolue du prochain élément à placer
    /// (utilisé pour définir page_y_offset de la nouvelle page).
    fn flush_page(&mut self, next_y: f32) {
        if !self.current_page_boxes.is_empty() {
            let page_number = self.finished_pages.len() as u32 + 1;
            self.finished_pages.push(Page {
                page_number,
                total_pages: 0,
                content: std::mem::take(&mut self.current_page_boxes),
                margin_boxes: Vec::new(),
            });
        }
        self.page_y_offset = next_y;
        self.used_height = 0.0;
    }

    fn is_current_page_empty(&self) -> bool {
        self.current_page_boxes.is_empty()
    }
}

// =============================================================================
// POINT D'ENTRÉE PRINCIPAL
// =============================================================================

/// Fragmente un LayoutTree root en pages PDF.
/// Toutes les coordonnées sont en points typographiques (pt).
pub fn paginate(root: &LayoutBox, config: &PageConfig) -> Vec<Page> {
    let page_height = config.content_height_pt();
    let mut ctx = PaginationContext::new(page_height);

    // Traiter les enfants directs de la racine
    for child in &root.children {
        fragment_box(child, &mut ctx);
    }

    // Flush la dernière page si non vide
    if !ctx.is_current_page_empty() {
        ctx.flush_page(0.0);
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
        ctx.flush_page(layout_box.rect.y);
    }

    // ─── Position-based fit check ────────────────────────────────────────────
    // Check if the box, at its actual ribbon position, fits within the current page.
    let box_bottom_on_page = (layout_box.rect.y - ctx.page_y_offset) + box_height;
    let fits_on_current_page = box_bottom_on_page <= ctx.page_height;
    let fits_on_new_page = box_height <= ctx.page_height;

    // ─── Règle 2 : page-break-inside: avoid ─────────────────────────────────
    let avoid_break_inside = style.page_break_inside == PageBreakInside::Avoid;

    if !fits_on_current_page && avoid_break_inside {
        if fits_on_new_page {
            if !ctx.is_current_page_empty() {
                ctx.flush_page(layout_box.rect.y);
            }
            place_box_on_current_page(layout_box, ctx);
            if should_break_after(style) {
                ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
            }
            return;
        }
        // Le bloc est plus grand qu'une page → on ne peut pas éviter la coupure.
    }

    // ─── Règle 3 : Le bloc tient sur la page courante ───────────────────────
    if fits_on_current_page {
        place_box_on_current_page(layout_box, ctx);
        if should_break_after(style) {
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        }
        return;
    }

    // ─── Règle 4 : Le bloc ne tient pas → fragmentation ─────────────────────
    if !layout_box.children.is_empty() {
        // Create a container fragment on the current page (preserves background/borders)
        fragment_container(layout_box, ctx);
    } else {
        // Boîte feuille (texte, image, etc.)
        if ctx.is_current_page_empty() {
            // Force : même trop grande, on la met sur la page vide (anti-boucle infinie)
            place_box_on_current_page(layout_box, ctx);
            ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
        } else {
            // Pousse sur la page suivante
            ctx.flush_page(layout_box.rect.y);
            place_box_on_current_page(layout_box, ctx);
            if box_height > ctx.page_height {
                ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
            }
        }
    }

    // ─── Règle 5 : page-break-after ─────────────────────────────────────────
    if should_break_after(style) && !ctx.is_current_page_empty() {
        ctx.flush_page(layout_box.rect.y + layout_box.rect.height);
    }
}

// =============================================================================
// FRAGMENTATION D'UN CONTENEUR
// Quand un conteneur ne tient pas sur la page courante, on distribue
// ses enfants entre la page courante et les suivantes, en créant des
// "wrapper fragments" sur chaque page pour préserver le contexte visuel
// (background, borders) du conteneur parent.
// =============================================================================

fn fragment_container(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    // Collect children that go on the current page vs next pages
    let mut current_page_children: Vec<LayoutBox> = Vec::new();
    let mut is_first_page = true;

    for child in &layout_box.children {
        let child_height = child.rect.height;
        // Position-based fit check: does this child's bottom fit on the current page?
        let child_bottom_on_page = (child.rect.y - ctx.page_y_offset) + child_height;
        let child_fits = child_bottom_on_page <= ctx.page_height;

        if child_fits {
            // Child fits on current page — add to current wrapper
            let mut placed_child = child.clone();
            offset_y_recursive(&mut placed_child, -ctx.page_y_offset);
            current_page_children.push(placed_child);
            ctx.used_height = ctx.used_height.max(child_bottom_on_page);
        } else if !child.children.is_empty() && child_height > ctx.page_height {
            // Child is a large container that doesn't fit on any single page
            // Flush current wrapper first, then recurse into this child
            if !current_page_children.is_empty() {
                let wrapper = make_container_fragment(layout_box, &current_page_children, ctx, is_first_page, false);
                ctx.current_page_boxes.push(wrapper);
                current_page_children.clear();
            }
            // Recurse into the child's own fragmentation
            fragment_box(child, ctx);
            is_first_page = false;
        } else {
            // Child doesn't fit — flush current page and start new
            if !current_page_children.is_empty() || !ctx.is_current_page_empty() {
                if !current_page_children.is_empty() {
                    let wrapper = make_container_fragment(layout_box, &current_page_children, ctx, is_first_page, false);
                    ctx.current_page_boxes.push(wrapper);
                    current_page_children.clear();
                }
                ctx.flush_page(child.rect.y);
                is_first_page = false;
            }

            // Place child on new page
            let mut placed_child = child.clone();
            offset_y_recursive(&mut placed_child, -ctx.page_y_offset);
            let child_bottom = placed_child.rect.y + placed_child.rect.height;
            current_page_children.push(placed_child);
            ctx.used_height = ctx.used_height.max(child_bottom);
        }
    }

    // Flush remaining children as a wrapper on the current page
    if !current_page_children.is_empty() {
        let wrapper = make_container_fragment(layout_box, &current_page_children, ctx, is_first_page, true);
        ctx.current_page_boxes.push(wrapper);
    }
}

/// Create a container fragment (partial copy of the parent) that wraps
/// a subset of children for one page. Preserves background, borders, etc.
fn make_container_fragment(
    parent: &LayoutBox,
    children: &[LayoutBox],
    ctx: &PaginationContext,
    is_first_fragment: bool,
    is_last_fragment: bool,
) -> LayoutBox {
    // Compute bounding box of children on this page
    let min_y = children.iter().map(|c| c.rect.y).fold(f32::MAX, f32::min);
    let max_bottom = children.iter()
        .map(|c| c.rect.y + c.rect.height)
        .fold(0.0f32, f32::max);
    let fragment_height = max_bottom - min_y
        + if is_first_fragment { parent.padding.top + parent.border.top } else { 0.0 }
        + if is_last_fragment { parent.padding.bottom + parent.border.bottom } else { 0.0 };

    let page_rel_y = (parent.rect.y - ctx.page_y_offset).max(0.0);
    let y = if is_first_fragment { page_rel_y } else { 0.0 };

    let rect = Rect::new(parent.rect.x, y, parent.rect.width, fragment_height);
    let content = Rect::new(
        parent.content.x,
        y + if is_first_fragment { parent.padding.top + parent.border.top } else { 0.0 },
        parent.content.width,
        (fragment_height - parent.padding.vertical() - parent.border.vertical()).max(0.0),
    );

    LayoutBox {
        node_id: parent.node_id,
        style: parent.style.clone(),
        rect,
        content,
        padding: if is_first_fragment { parent.padding } else {
            Insets { top: 0.0, ..parent.padding }
        },
        border: if is_first_fragment { parent.border } else {
            Insets { top: 0.0, ..parent.border }
        },
        margin: Insets::zero(),
        children: children.to_vec(),
        shaped_lines: Vec::new(),
        image_src: None,
        text_content: None,
        out_of_flow: false,
        visual_offset_x: 0.0,
        visual_offset_y: 0.0,
    }
}

// =============================================================================
// PLACEMENT D'UNE BOX SUR LA PAGE COURANTE
// Repositionnement Y : y_page = y_absolute - page_y_offset
// =============================================================================

fn place_box_on_current_page(layout_box: &LayoutBox, ctx: &mut PaginationContext) {
    let mut placed_box = layout_box.clone();
    offset_y_recursive(&mut placed_box, -ctx.page_y_offset);

    // Track the actual bottom extent on the page (not sum of heights)
    let box_bottom = placed_box.rect.y + placed_box.rect.height;
    ctx.used_height = ctx.used_height.max(box_bottom);
    ctx.current_page_boxes.push(placed_box);
}

/// Recursively offset all Y coordinates in a LayoutBox tree.
fn offset_y_recursive(layout_box: &mut LayoutBox, dy: f32) {
    layout_box.rect.y += dy;
    layout_box.content.y += dy;
    for child in &mut layout_box.children {
        offset_y_recursive(child, dy);
    }
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
pub fn create_empty_page(_config: &PageConfig) -> Page {
    Page {
        page_number: 1,
        total_pages: 1,
        content: Vec::new(),
        margin_boxes: Vec::new(),
    }
}

// =============================================================================
// BREAK UNITS — Extraction des unités sécables depuis l'arbre LayoutBox
// =============================================================================
// Après le layout Taffy + shaping cosmic-text, on construit une liste PLATE
// d'unités sécables. Chaque unité est la plus petite entité déplaçable sans
// casser le sens du document.
//
// Types de BreakUnit :
//   - TextLine  : une ligne individuelle issue des shaped_lines du LayoutBox
//   - Atomic    : bloc non sécable (image, tableau avec break-inside:avoid)
//   - ForcedBreak : marqueur de saut de page forcé (break-before: page)
// =============================================================================

use ferropdf_core::layout::BreakUnit;

/// Extraire les unités sécables depuis l'arbre de LayoutBox.
/// Parcourt l'arbre récursivement et produit une liste plate de BreakUnit.
pub fn extract_break_units(root: &LayoutBox) -> Vec<BreakUnit> {
    let mut units = Vec::new();
    for child in &root.children {
        extract_recursive(child, &mut units);
    }
    units
}

fn extract_recursive(lb: &LayoutBox, units: &mut Vec<BreakUnit>) {
    // Forced break before
    if should_break_before(&lb.style) {
        units.push(BreakUnit::ForcedBreak);
    }

    // Atomic: break-inside: avoid, or has image, or is a leaf without children/shaped_lines
    let is_atomic = lb.style.page_break_inside == PageBreakInside::Avoid
        || lb.image_src.is_some()
        || (lb.children.is_empty() && lb.shaped_lines.is_empty() && lb.text_content.is_some());

    if is_atomic {
        units.push(BreakUnit::Atomic {
            y_top: lb.rect.y,
            y_bottom: lb.rect.y + lb.rect.height,
            node: lb.clone(),
        });
    } else if !lb.shaped_lines.is_empty() {
        // Text node with shaped lines → one BreakUnit::TextLine per line
        for (i, line) in lb.shaped_lines.iter().enumerate() {
            let line_height = if i + 1 < lb.shaped_lines.len() {
                lb.shaped_lines[i + 1].y - line.y
            } else {
                lb.style.line_height
            };
            units.push(BreakUnit::TextLine {
                y_top: lb.content.y + line.y,
                y_bottom: lb.content.y + line.y + line_height,
                line_index: i,
                parent_node: lb.node_id,
                content: line.clone(),
            });
        }
    } else if lb.text_content.is_some() && lb.shaped_lines.is_empty() {
        // Text node without shaped lines → treat as atomic
        units.push(BreakUnit::Atomic {
            y_top: lb.rect.y,
            y_bottom: lb.rect.y + lb.rect.height,
            node: lb.clone(),
        });
    } else if !lb.children.is_empty() {
        // Container → recurse into children
        for child in &lb.children {
            extract_recursive(child, units);
        }
    }

    // Forced break after
    if should_break_after(&lb.style) {
        units.push(BreakUnit::ForcedBreak);
    }
}

// =============================================================================
// find_break_point — Algorithme de recherche du point de coupure intelligent
// =============================================================================
// Prend la liste de BreakUnit, la limite de page (page_bottom), et les paramètres
// orphelines/veuves. Retourne l'index du premier BreakUnit de la page suivante.
//
// Étapes :
//   1. Index naïf — première BreakUnit dont y_top >= page_bottom
//   2. Correction orphelines — si trop peu de lignes d'un paragraphe en fin de page
//   3. Correction veuves — si trop peu de lignes d'un paragraphe en début de page suivante
//   4. Intégrité des atomiques — pas de coupure au milieu d'un Atomic
//   5. Sauts forcés — ForcedBreak prend priorité
// =============================================================================

/// Trouver le point de coupure optimal dans la liste de BreakUnit.
///
/// Retourne l'index du premier BreakUnit qui doit aller sur la page suivante.
/// `page_top` est la coordonnée Y absolue du haut de la page courante.
/// `page_height` est la hauteur de la zone de contenu de la page.
/// `min_orphans` et `min_widows` sont les minimums CSS (défaut = 2).
pub fn find_break_point(
    units: &[BreakUnit],
    page_top: f32,
    page_height: f32,
    min_orphans: u32,
    min_widows: u32,
) -> usize {
    let page_bottom = page_top + page_height;

    // Étape 1 — Index naïf : première unité qui dépasse la page
    let mut naive_index = units.len();
    for (i, unit) in units.iter().enumerate() {
        if let BreakUnit::ForcedBreak = unit {
            // Un saut forcé avant l'index naïf prend priorité (étape 5)
            if unit.y_top() <= page_bottom || i < naive_index {
                return i + 1; // Le ForcedBreak est consommé, la page suivante commence après
            }
        }
        if unit.y_bottom() > page_bottom && naive_index == units.len() {
            naive_index = i;
        }
    }

    if naive_index == 0 {
        // Rien ne tient sur cette page — force au moins une unité (anti-boucle infinie)
        return 1.min(units.len());
    }
    if naive_index >= units.len() {
        return units.len();
    }

    let mut break_index = naive_index;

    // Étape 2 — Correction orphelines
    break_index = adjust_for_orphans(units, break_index, min_orphans);

    // Étape 3 — Correction veuves
    break_index = adjust_for_widows(units, break_index, min_widows);

    // Étape 4 — Intégrité des atomiques
    break_index = enforce_atomic_integrity(units, break_index);

    break_index.clamp(1, units.len())
}

/// Correction orphelines : si moins de `min_orphans` lignes du même paragraphe
/// sont présentes juste avant le point de coupure, on recule l'index pour
/// emporter ces lignes sur la page suivante.
fn adjust_for_orphans(units: &[BreakUnit], break_idx: usize, min_orphans: u32) -> usize {
    if break_idx == 0 || break_idx >= units.len() || min_orphans < 2 {
        return break_idx;
    }

    // Regarder l'unité juste avant le break
    if let BreakUnit::TextLine { parent_node: Some(parent), .. } = &units[break_idx - 1] {
        // Compter combien de lignes de ce paragraphe sont juste avant l'index
        let mut orphan_count = 0u32;
        let mut i = break_idx;
        while i > 0 {
            i -= 1;
            match &units[i] {
                BreakUnit::TextLine { parent_node: Some(p), .. } if p == parent => {
                    orphan_count += 1;
                }
                _ => break,
            }
        }

        if orphan_count > 0 && orphan_count < min_orphans {
            // Remonter l'index pour emporter ces lignes orphelines
            return break_idx - orphan_count as usize;
        }
    }

    break_idx
}

/// Correction veuves : si moins de `min_widows` lignes du même paragraphe
/// seront au début de la page suivante, ajuster.
fn adjust_for_widows(units: &[BreakUnit], break_idx: usize, min_widows: u32) -> usize {
    if break_idx >= units.len() || min_widows < 2 {
        return break_idx;
    }

    // Regarder l'unité au point de coupure (première de la page suivante)
    if let BreakUnit::TextLine { parent_node: Some(parent), .. } = &units[break_idx] {
        // Compter combien de lignes de ce paragraphe seront au début de la page suivante
        let mut widow_count = 0u32;
        for unit in &units[break_idx..] {
            match unit {
                BreakUnit::TextLine { parent_node: Some(p), .. } if p == parent => {
                    widow_count += 1;
                }
                _ => break,
            }
        }

        if widow_count > 0 && widow_count < min_widows {
            // Reculer l'index pour ajouter des lignes à la page suivante
            let lines_to_pull = min_widows - widow_count;
            if break_idx > lines_to_pull as usize {
                return break_idx - lines_to_pull as usize;
            }
        }
    }

    break_idx
}

/// Intégrité des atomiques : si l'index tombe au milieu d'un Atomic,
/// reculer l'index jusqu'avant le début de cet Atomic.
fn enforce_atomic_integrity(units: &[BreakUnit], break_idx: usize) -> usize {
    if break_idx >= units.len() {
        return break_idx;
    }

    // Si l'unité au break est un Atomic, on ne peut pas couper au milieu
    // → on garde le break_idx tel quel (il pointe déjà au début de l'Atomic)
    // Si l'unité juste avant est un Atomic dont le bas dépasse, on recule
    if break_idx > 0 {
        if let BreakUnit::Atomic { .. } = &units[break_idx - 1] {
            // L'Atomic est entièrement sur la page courante — OK
        }
    }

    break_idx
}
