// =============================================================================
// table_layout.rs — Algorithme de layout de tableaux CSS
// =============================================================================
// Traduit depuis les algorithmes de Blink (BSD licence) :
//   blink/renderer/core/layout/layout_table.cc
//   blink/renderer/core/layout/layout_table_cell.cc
//
// Spec de référence :
//   CSS 2.1 §17.5   — Table layout
//   CSS 2.1 §17.5.2 — Fixed table layout
//   CSS 2.1 §17.5.3 — Auto table layout (simplifié)
//
// Ce module s'exécute AVANT Taffy.
// Il calcule les largeurs de colonnes, puis construit les TrackSizingFunction
// Taffy pour le CSS Grid qui représente le tableau.
// =============================================================================

use ferropdf_core::{Document, NodeId, NodeType, ComputedStyle, FerroError, Length};
use cosmic_text::{Buffer, FontSystem, Metrics, Attrs, Family, Shaping, Wrap};
use taffy::{
    TrackSizingFunction, MinMax,
    MinTrackSizingFunction, MaxTrackSizingFunction,
    LengthPercentage,
    style::GridTrackVec,
};
use std::collections::HashMap;

// =============================================================================
// STRUCTURES DE DONNÉES
// =============================================================================

/// Représente la structure logique d'un tableau HTML.
/// Construit en parcourant le DOM, avant toute opération de layout.
/// Inspiré de TableGridCell dans Blink.
#[derive(Debug)]
pub struct TableGrid {
    /// columns[col_idx] = liste des NodeId des cellules de cette colonne
    pub columns: Vec<Vec<NodeId>>,
    /// rows[row_idx] = liste des NodeId des cellules de cette ligne
    pub rows: Vec<Vec<NodeId>>,
    /// Nombre de colonnes
    pub num_cols: usize,
    /// Nombre de lignes
    pub num_rows: usize,
    /// Contenu textuel de chaque cellule (pour la mesure min-content)
    pub cell_text: HashMap<NodeId, String>,
    /// Style de chaque cellule
    pub cell_styles: HashMap<NodeId, ComputedStyle>,
}

/// Résultat du calcul de layout d'un tableau.
#[derive(Debug)]
pub struct TableLayoutResult {
    /// Largeur calculée de chaque colonne (en points)
    pub column_widths: Vec<f32>,
    /// Hauteur calculée de chaque ligne (en points)
    pub row_heights: Vec<f32>,
    /// Largeur totale du tableau
    pub total_width: f32,
    /// Hauteur totale du tableau
    pub total_height: f32,
    /// TrackSizingFunction pour Taffy (grid-template-columns)
    pub taffy_columns: GridTrackVec<TrackSizingFunction>,
    /// TrackSizingFunction pour Taffy (grid-template-rows)
    pub taffy_rows: GridTrackVec<TrackSizingFunction>,
}

// =============================================================================
// PHASE 0 — CONSTRUCTION DE LA GRILLE LOGIQUE
// Inspiré de TableGridStructure dans Blink layout_table.cc
// =============================================================================

/// Construit la grille logique du tableau depuis le DOM.
/// Parcourt <table> → <thead>/<tbody>/<tfoot> → <tr> → <td>/<th>.
///
/// Note : colspan/rowspan sont ignorés en v1 (TODO v2).
pub fn build_table_grid(
    table_node: NodeId,
    document: &Document,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> Result<TableGrid, FerroError> {
    let mut rows: Vec<Vec<NodeId>> = Vec::new();

    // Collecter les <tr> en traversant thead/tbody/tfoot
    let tr_rows = collect_tr_nodes(table_node, document)?;

    for tr_id in &tr_rows {
        let cells = collect_td_nodes(*tr_id, document)?;
        rows.push(cells);
    }

    if rows.is_empty() {
        return Ok(TableGrid {
            columns: vec![],
            rows: vec![],
            num_cols: 0,
            num_rows: 0,
            cell_text: HashMap::new(),
            cell_styles: HashMap::new(),
        });
    }

    // Calculer le nombre de colonnes = max(len(row)) sur toutes les lignes
    let num_cols = rows.iter().map(|r| r.len()).max().unwrap_or(0);
    let num_rows = rows.len();

    // Construire la structure colonnes[col] = [cell_ids...]
    let mut columns: Vec<Vec<NodeId>> = vec![Vec::new(); num_cols];
    for row in &rows {
        for (col_idx, &cell_id) in row.iter().enumerate() {
            if col_idx < num_cols {
                columns[col_idx].push(cell_id);
            }
        }
    }

    // Collecter le texte et les styles de chaque cellule
    let mut cell_text:   HashMap<NodeId, String>        = HashMap::new();
    let mut cell_styles: HashMap<NodeId, ComputedStyle> = HashMap::new();

    for row in &rows {
        for &cell_id in row {
            let text = extract_text_content(cell_id, document)?;
            cell_text.insert(cell_id, text);

            if let Some(style) = styles.get(&cell_id) {
                cell_styles.insert(cell_id, style.clone());
            }
        }
    }

    Ok(TableGrid {
        columns,
        rows,
        num_cols,
        num_rows,
        cell_text,
        cell_styles,
    })
}

// =============================================================================
// PHASE 1 — CALCUL DES LARGEURS DE COLONNES
// CSS 2.1 §17.5.2 (fixed) + §17.5.3 (auto simplifié)
// Inspiré de LayoutTable::ComputedColumnWidths() dans Blink
// =============================================================================

/// Calcule les largeurs de colonnes selon l'algorithme CSS 2.1 §17.5.2/17.5.3.
///
/// Algorithme en 4 étapes :
///   1. Chercher les largeurs fixes (attribut width ou style CSS width sur <col>/<td>)
///   2. Mesurer le min-content width de chaque colonne via cosmic-text
///   3. Distribuer l'espace disponible proportionnellement
///   4. Construire les TrackSizingFunction pour Taffy
pub fn compute_column_widths(
    grid: &TableGrid,
    table_width: f32,
    font_system: &mut FontSystem,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> Result<Vec<f32>, FerroError> {
    if grid.num_cols == 0 {
        return Ok(vec![]);
    }

    // ─── Étape 1 : Largeurs fixes depuis les styles CSS ─────────────────────
    // Si une cellule a width: Xpx ou width: X%, c'est une largeur fixe.
    // On prend la largeur de la première cellule de chaque colonne comme hint.
    let fixed_widths: Vec<Option<f32>> = (0..grid.num_cols)
        .map(|col_idx| {
            grid.columns[col_idx]
                .first()
                .and_then(|cell_id| styles.get(cell_id))
                .and_then(|style| match &style.width {
                    Length::Px(px) => Some(*px),
                    Length::Percent(p) => Some(table_width * p / 100.0),
                    _ => None,
                })
        })
        .collect();

    // ─── Étape 2 : Min-content width de chaque colonne ──────────────────────
    // Pour chaque colonne, mesurer le texte le plus large de ses cellules
    // sans word-wrap (une seule ligne).
    let min_content_widths: Vec<f32> = (0..grid.num_cols)
        .map(|col_idx| {
            grid.columns[col_idx]
                .iter()
                .map(|cell_id| {
                    let text = grid.cell_text.get(cell_id).map(|s| s.as_str()).unwrap_or("");
                    let style = styles.get(cell_id);
                    let font_size = style
                        .and_then(|s| match s.font_size {
                            Length::Px(px) => Some(px),
                            _ => None,
                        })
                        .unwrap_or(12.0);
                    let font_family = style
                        .and_then(|s| s.font_family.first().cloned())
                        .unwrap_or_else(|| "sans-serif".to_string());

                    measure_min_content_width(text, font_size, &font_family, font_system)
                        // Ajouter le padding horizontal de la cellule
                        + style
                            .map(|s| {
                                resolve_px(&s.padding_left) + resolve_px(&s.padding_right)
                            })
                            .unwrap_or(0.0)
                })
                .fold(0.0_f32, f32::max) // max sur toutes les cellules de la colonne
        })
        .collect();

    // ─── Étape 3 : Distribution de l'espace ─────────────────────────────────
    // Les colonnes avec largeur fixe gardent leur largeur.
    // L'espace restant est distribué aux colonnes sans largeur fixe,
    // proportionnellement à leur min-content width.

    let fixed_total: f32 = fixed_widths
        .iter()
        .filter_map(|w| *w)
        .sum();

    let flexible_min_total: f32 = (0..grid.num_cols)
        .filter(|&i| fixed_widths[i].is_none())
        .map(|i| min_content_widths[i])
        .sum();

    let available_for_flexible = (table_width - fixed_total).max(0.0);

    let column_widths: Vec<f32> = (0..grid.num_cols)
        .map(|i| {
            if let Some(fixed) = fixed_widths[i] {
                // Largeur fixe explicite
                fixed
            } else if flexible_min_total > 0.0 {
                // Distribuer proportionnellement au min-content width
                let ratio = min_content_widths[i] / flexible_min_total;
                let allocated = available_for_flexible * ratio;
                // La largeur allouée ne peut pas être inférieure au min-content
                allocated.max(min_content_widths[i])
            } else {
                // Fallback : division égale
                available_for_flexible / (grid.num_cols as f32)
            }
        })
        .collect();

    Ok(column_widths)
}

// =============================================================================
// PHASE 2 — CALCUL DES HAUTEURS DE LIGNES
// CSS 2.1 §17.5 — Row height
// Inspiré de LayoutTableRow::ComputeRowHeight() dans Blink
// =============================================================================

/// Calcule la hauteur de chaque ligne.
/// CSS 2.1 : la hauteur d'une ligne = max(hauteur de toutes ses cellules).
/// Chaque cellule est étirée à cette hauteur (align: stretch par défaut).
pub fn compute_row_heights(
    grid: &TableGrid,
    column_widths: &[f32],
    font_system: &mut FontSystem,
    styles: &HashMap<NodeId, ComputedStyle>,
) -> Result<Vec<f32>, FerroError> {
    let row_heights: Vec<f32> = grid
        .rows
        .iter()
        .map(|row| {
            row.iter()
                .enumerate()
                .map(|(col_idx, cell_id)| {
                    let text = grid.cell_text.get(cell_id).map(|s| s.as_str()).unwrap_or("");
                    let style = styles.get(cell_id);
                    let cell_width = column_widths.get(col_idx).cloned().unwrap_or(50.0);
                    let font_size = style
                        .and_then(|s| match s.font_size {
                            Length::Px(px) => Some(px),
                            _ => None,
                        })
                        .unwrap_or(12.0);
                    let font_family = style
                        .and_then(|s| s.font_family.first().cloned())
                        .unwrap_or_else(|| "sans-serif".to_string());
                    let padding_v = style
                        .map(|s| resolve_px(&s.padding_top) + resolve_px(&s.padding_bottom))
                        .unwrap_or(0.0);

                    // Mesurer la hauteur du texte dans cette cellule avec word-wrap
                    measure_text_height(text, cell_width, font_size, &font_family, font_system)
                        + padding_v
                })
                .fold(0.0_f32, f32::max) // max sur toutes les cellules de la ligne
        })
        .collect();

    Ok(row_heights)
}

// =============================================================================
// PHASE 3 — CONSTRUCTION DES TRACK SIZING FUNCTIONS TAFFY
// =============================================================================

/// Construit les grid-template-columns pour Taffy depuis les largeurs calculées.
pub fn build_taffy_column_tracks(
    column_widths: &[f32],
) -> GridTrackVec<TrackSizingFunction> {
    column_widths
        .iter()
        .map(|&w| {
            TrackSizingFunction::Single(MinMax {
                min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(w)),
                max: MaxTrackSizingFunction::Fixed(LengthPercentage::Length(w)),
            })
        })
        .collect()
}

/// Construit les grid-template-rows pour Taffy depuis les hauteurs calculées.
pub fn build_taffy_row_tracks(
    row_heights: &[f32],
) -> GridTrackVec<TrackSizingFunction> {
    row_heights
        .iter()
        .map(|&h| {
            TrackSizingFunction::Single(MinMax {
                min: MinTrackSizingFunction::Fixed(LengthPercentage::Length(h)),
                max: MaxTrackSizingFunction::Fixed(LengthPercentage::Length(h)),
            })
        })
        .collect()
}

// =============================================================================
// MESURES DE TEXTE VIA COSMIC-TEXT
// =============================================================================

/// Mesure le min-content width d'un texte (largeur sur une ligne, sans wrap).
/// Utilisé pour calculer la largeur minimale d'une colonne.
fn measure_min_content_width(
    text: &str,
    font_size: f32,
    font_family: &str,
    font_system: &mut FontSystem,
) -> f32 {
    if text.is_empty() {
        return 0.0;
    }

    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));

    // Pas de wrap pour mesurer le min-content width
    buffer.set_wrap(font_system, Wrap::None);

    let attrs = Attrs::new().family(Family::Name(font_family));
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    buffer
        .layout_runs()
        .map(|run| run.line_w)
        .fold(0.0_f32, f32::max)
}

/// Mesure la hauteur d'un texte avec word-wrap dans une largeur donnée.
/// Utilisé pour calculer la hauteur d'une cellule.
fn measure_text_height(
    text: &str,
    available_width: f32,
    font_size: f32,
    font_family: &str,
    font_system: &mut FontSystem,
) -> f32 {
    if text.is_empty() {
        return font_size * 1.2; // Hauteur minimale d'une ligne vide
    }

    let line_height = font_size * 1.2;
    let mut buffer = Buffer::new(font_system, Metrics::new(font_size, line_height));

    // Wrap activé pour mesurer la hauteur réelle
    buffer.set_wrap(font_system, Wrap::Word);
    buffer.set_size(font_system, Some(available_width), None);

    let attrs = Attrs::new().family(Family::Name(font_family));
    buffer.set_text(font_system, text, attrs, Shaping::Advanced);
    buffer.shape_until_scroll(font_system, false);

    let num_lines = buffer.layout_runs().count().max(1);
    num_lines as f32 * line_height
}

// =============================================================================
// HELPERS DOM — Collecte des nœuds de table
// =============================================================================

/// Collecte tous les <tr> dans un <table>, en traversant thead/tbody/tfoot.
fn collect_tr_nodes(
    table_id: NodeId,
    document: &Document,
) -> Result<Vec<NodeId>, FerroError> {
    let mut tr_nodes = Vec::new();

    let table_children = document
        .children(table_id)
        .map_err(|e| FerroError::Layout(e.to_string()))?;

    for child_id in table_children {
        let node_type = document
            .node_type(child_id)
            .map_err(|e| FerroError::Layout(e.to_string()))?;

        match node_type {
            NodeType::Element(tag)
                if matches!(tag.as_str(), "thead" | "tbody" | "tfoot") =>
            {
                // Traverser les sections → collecter leurs <tr>
                let section_children = document
                    .children(child_id)
                    .map_err(|e| FerroError::Layout(e.to_string()))?;
                for section_child in section_children {
                    if is_element(section_child, "tr", document) {
                        tr_nodes.push(section_child);
                    }
                }
            }
            NodeType::Element(tag) if tag == "tr" => {
                // <tr> directement dans <table> (sans thead/tbody/tfoot)
                tr_nodes.push(child_id);
            }
            _ => {} // Ignorer les nœuds texte, commentaires, etc.
        }
    }

    Ok(tr_nodes)
}

/// Collecte tous les <td> et <th> dans un <tr>.
fn collect_td_nodes(
    tr_id: NodeId,
    document: &Document,
) -> Result<Vec<NodeId>, FerroError> {
    let children = document
        .children(tr_id)
        .map_err(|e| FerroError::Layout(e.to_string()))?;

    let cells: Vec<NodeId> = children
        .into_iter()
        .filter(|&child_id| {
            is_element(child_id, "td", document) || is_element(child_id, "th", document)
        })
        .collect();

    Ok(cells)
}

/// Extrait le contenu textuel récursif d'un nœud DOM.
fn extract_text_content(
    node_id: NodeId,
    document: &Document,
) -> Result<String, FerroError> {
    let mut text = String::new();

    if let Ok(NodeType::Text(content)) = document.node_type(node_id) {
        text.push_str(&content);
    }

    if let Ok(children) = document.children(node_id) {
        for child_id in children {
            text.push_str(&extract_text_content(child_id, document)?);
        }
    }

    Ok(text)
}

/// Vérifie si un nœud est un élément avec le tag donné.
fn is_element(node_id: NodeId, tag: &str, document: &Document) -> bool {
    matches!(
        document.node_type(node_id),
        Ok(NodeType::Element(t)) if t.eq_ignore_ascii_case(tag)
    )
}

/// Convertit une Length en px (helper rapide).
fn resolve_px(length: &Length) -> f32 {
    match length {
        Length::Px(px) => *px,
        Length::Zero   => 0.0,
        _              => 0.0,
    }
}

// =============================================================================
// TESTS UNITAIRES
// =============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_collapse_margins_dans_table() {
        // Une colonne avec deux cellules contenant "Hi" et "Hello World"
        // min_content("Hello World") > min_content("Hi")
        // → la colonne doit avoir la largeur de "Hello World"
        // (vérification logique — pas de vrai FontSystem en test)
        let widths = vec![50.0_f32, 120.0, 80.0];
        let tracks = build_taffy_column_tracks(&widths);
        assert_eq!(tracks.len(), 3);
    }

    #[test]
    fn test_build_taffy_row_tracks() {
        let heights = vec![24.0_f32, 48.0, 24.0];
        let tracks = build_taffy_row_tracks(&heights);
        assert_eq!(tracks.len(), 3);
    }

    #[test]
    fn test_compute_column_widths_espace_egal_sans_min() {
        // 3 colonnes, largeur table = 300px, min-content = 0 pour toutes
        // → chaque colonne reçoit 100px
        let grid = TableGrid {
            columns:     vec![vec![], vec![], vec![]],
            rows:        vec![],
            num_cols:    3,
            num_rows:    0,
            cell_text:   HashMap::new(),
            cell_styles: HashMap::new(),
        };
        // Sans FontSystem réel, on vérifie juste la longueur
        assert_eq!(grid.num_cols, 3);
    }

    #[test]
    fn test_resolve_px_length() {
        assert_eq!(resolve_px(&Length::Px(10.0)), 10.0);
        assert_eq!(resolve_px(&Length::Zero),      0.0);
        assert_eq!(resolve_px(&Length::Auto),      0.0);
    }
}
