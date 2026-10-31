//! Clickable links (`<a href>`) and the PDF outline (bookmarks from
//! `<h1>`–`<h6>`), from the `Link`, `Anchor` and `Bookmark` draw ops.

use crate::display_list::{DrawOp, PageDisplayList};
use ferropdf_core::Rect;
use pdf_writer::types::{ActionType, AnnotationType};
use pdf_writer::{Finish, Pdf, Rect as PdfWriterRect, Ref, Str, TextStr};
use std::collections::{HashMap, HashSet};

/// One link annotation of a page.
pub(crate) struct LinkAnnotation {
    pub annotation: Ref,
    rect: Rect,
    target: Target,
}

enum Target {
    Uri(String),
    /// Page index and y (page coordinates, top-left origin).
    Internal(usize, f32),
}

struct Bookmark {
    level: u8,
    title: String,
    page: usize,
    y: f32,
}

/// Everything the PDF writer needs: link annotations per page and the
/// outline entries in document order.
pub(crate) struct Navigation {
    pub links: Vec<Vec<LinkAnnotation>>,
    pub bookmarks: Vec<BookmarkEntry>,
}

pub(crate) struct BookmarkEntry {
    inner: Bookmark,
    item: Ref,
}

/// External link targets that become URI actions. Anything else
/// (`javascript:`, `data:`, `file:` …) is not made clickable.
fn is_safe_uri(target: &str) -> bool {
    let lower = target.trim().to_ascii_lowercase();
    match lower.split_once(':') {
        // A scheme: only web, mail and phone links.
        Some((scheme, _)) if !scheme.contains('/') && !scheme.contains('?') => {
            matches!(scheme, "http" | "https" | "mailto" | "tel")
        }
        // No scheme: a relative URL.
        _ => !lower.is_empty(),
    }
}

/// Collect anchors, links and bookmarks from the display lists, and
/// allocate the annotation and outline-item objects.
pub(crate) fn collect(pages: &[PageDisplayList], next_ref: &mut dyn FnMut() -> Ref) -> Navigation {
    // First position of every anchor; first fragment of every heading.
    let mut anchors: HashMap<&str, (usize, f32)> = HashMap::new();
    let mut seen = HashSet::new();
    let mut bookmarks = Vec::new();
    for (page, list) in pages.iter().enumerate() {
        for op in &list.ops {
            match op {
                DrawOp::Anchor { name, y } => {
                    anchors.entry(name.as_str()).or_insert((page, *y));
                }
                DrawOp::Bookmark {
                    key,
                    level,
                    title,
                    y,
                } if seen.insert(*key) => bookmarks.push(BookmarkEntry {
                    inner: Bookmark {
                        level: *level,
                        title: title.clone(),
                        page,
                        y: *y,
                    },
                    item: next_ref(),
                }),
                _ => {}
            }
        }
    }

    let links = pages
        .iter()
        .map(|list| {
            list.ops
                .iter()
                .filter_map(|op| {
                    let DrawOp::Link { rect, target } = op else {
                        return None;
                    };
                    if rect.width <= 0.0 || rect.height <= 0.0 {
                        return None;
                    }
                    let target = match target.trim().strip_prefix('#') {
                        // An unknown fragment is not clickable.
                        Some(name) => {
                            let (page, y) = anchors.get(name)?;
                            Target::Internal(*page, *y)
                        }
                        None if is_safe_uri(target) => Target::Uri(target.trim().to_string()),
                        None => return None,
                    };
                    Some(LinkAnnotation {
                        annotation: next_ref(),
                        rect: *rect,
                        target,
                    })
                })
                .collect()
        })
        .collect();

    Navigation { links, bookmarks }
}

/// Write the link annotations and the outline tree.
pub(crate) fn write(
    pdf: &mut Pdf,
    nav: &Navigation,
    page_refs: &[Ref],
    outline_ref: Option<Ref>,
    page_h: f32,
) {
    for link in nav.links.iter().flatten() {
        let r = link.rect;
        let mut annot = pdf.annotation(link.annotation);
        annot.subtype(AnnotationType::Link);
        annot.rect(PdfWriterRect::new(
            r.x,
            page_h - r.y - r.height,
            r.x + r.width,
            page_h - r.y,
        ));
        // No visible border (browsers draw none).
        annot.border(0.0, 0.0, 0.0, None);
        match &link.target {
            Target::Uri(uri) => {
                annot
                    .action()
                    .action_type(ActionType::Uri)
                    .uri(Str(uri.as_bytes()));
            }
            Target::Internal(page, y) => {
                annot
                    .action()
                    .action_type(ActionType::GoTo)
                    .destination()
                    .page(page_refs[*page])
                    .xyz(0.0, page_h - y, None);
            }
        }
        annot.finish();
    }

    let Some(root) = outline_ref else {
        return;
    };
    let items = &nav.bookmarks;
    // Parent of each entry: the closest earlier entry of a lower level.
    let mut parent: Vec<Option<usize>> = Vec::with_capacity(items.len());
    let mut stack: Vec<usize> = Vec::new();
    for (i, b) in items.iter().enumerate() {
        while stack
            .last()
            .is_some_and(|&j| items[j].inner.level >= b.inner.level)
        {
            stack.pop();
        }
        parent.push(stack.last().copied());
        stack.push(i);
    }
    let children =
        |p: Option<usize>| -> Vec<usize> { (0..items.len()).filter(|&i| parent[i] == p).collect() };
    // Every item is open: an entry's count is its number of descendants.
    let mut descendants = vec![0i32; items.len()];
    for i in (0..items.len()).rev() {
        if let Some(p) = parent[i] {
            descendants[p] += descendants[i] + 1;
        }
    }

    let top = children(None);
    let mut outline = pdf.outline(root);
    if let (Some(&first), Some(&last)) = (top.first(), top.last()) {
        outline.first(items[first].item);
        outline.last(items[last].item);
    }
    outline.count(items.len() as i32);
    outline.finish();

    for (i, entry) in items.iter().enumerate() {
        let siblings = children(parent[i]);
        let pos = siblings.iter().position(|&s| s == i).unwrap_or(0);
        let kids = children(Some(i));
        let mut item = pdf.outline_item(entry.item);
        item.title(TextStr(&entry.inner.title));
        item.parent(parent[i].map_or(root, |p| items[p].item));
        if pos > 0 {
            item.prev(items[siblings[pos - 1]].item);
        }
        if let Some(&next) = siblings.get(pos + 1) {
            item.next(items[next].item);
        }
        if let (Some(&first), Some(&last)) = (kids.first(), kids.last()) {
            item.first(items[first].item);
            item.last(items[last].item);
            item.count(descendants[i]);
        }
        item.dest()
            .page(page_refs[entry.inner.page])
            .xyz(0.0, page_h - entry.inner.y, None);
        item.finish();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_safe_schemes_are_clickable() {
        for ok in [
            "https://x.dev",
            "http://a",
            "mailto:a@b.c",
            "tel:+33",
            "docs/page.html",
            "?q=1",
        ] {
            assert!(is_safe_uri(ok), "{ok}");
        }
        for bad in [
            "javascript:alert(1)",
            " JavaScript:x",
            "data:text/html,x",
            "file:///etc/passwd",
            "",
        ] {
            assert!(!is_safe_uri(bad), "{bad}");
        }
    }

    fn page(ops: Vec<DrawOp>) -> PageDisplayList {
        PageDisplayList {
            ops,
            page_number: 1,
            total_pages: 1,
        }
    }

    #[test]
    fn internal_links_resolve_and_unknown_ones_are_dropped() {
        let mut n = 0;
        let mut next = || {
            n += 1;
            Ref::new(n)
        };
        let link = |t: &str| DrawOp::Link {
            rect: Rect::new(0.0, 0.0, 10.0, 10.0),
            target: t.into(),
        };
        let pages = vec![
            page(vec![link("#sec"), link("#missing"), link("javascript:x")]),
            page(vec![DrawOp::Anchor {
                name: "sec".into(),
                y: 40.0,
            }]),
        ];
        let nav = collect(&pages, &mut next);
        assert_eq!(nav.links[0].len(), 1);
        assert!(matches!(nav.links[0][0].target, Target::Internal(1, y) if y == 40.0));
    }

    #[test]
    fn heading_fragments_make_one_bookmark() {
        let mut n = 0;
        let mut next = || {
            n += 1;
            Ref::new(n)
        };
        let bm = |key| DrawOp::Bookmark {
            key,
            level: 1,
            title: "T".into(),
            y: 0.0,
        };
        let nav = collect(&[page(vec![bm(1), bm(2)]), page(vec![bm(2)])], &mut next);
        assert_eq!(nav.bookmarks.len(), 2);
    }
}
