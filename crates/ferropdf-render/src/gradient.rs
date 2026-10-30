//! Resolution of CSS gradients against a box, and their PDF encoding as
//! axial (type 2) / radial (type 3) shadings. Stops with alpha get a
//! luminosity soft mask built from a grey shading of the same geometry.

use ferropdf_core::{
    Color, ColorStop, Gradient, GradientDirection, GradientLength, RadialShape, RadialSize, Rect,
};
use pdf_writer::types::{FunctionShadingType, MaskType};
use pdf_writer::{Content, Finish, Name, Pdf, Rect as PdfWriterRect, Ref};

/// Geometry of a resolved gradient, in page coordinates (top-left origin, pt).
#[derive(Debug, Clone, PartialEq)]
pub enum GradientGeometry {
    Axial {
        x0: f32,
        y0: f32,
        x1: f32,
        y1: f32,
    },
    /// Ellipse centred on (cx, cy); offset 1.0 lies on the ellipse (rx, ry).
    Radial {
        cx: f32,
        cy: f32,
        rx: f32,
        ry: f32,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct ResolvedGradient {
    pub geometry: GradientGeometry,
    /// Offsets in [0, 1], non-decreasing; first is 0 and last is 1. Equal
    /// neighbouring offsets form a hard colour change.
    pub stops: Vec<(f32, Color)>,
}

impl ResolvedGradient {
    pub fn has_alpha(&self) -> bool {
        self.stops.iter().any(|(_, c)| c.a < 0.999)
    }
}

/// Resolve `g` for a gradient box `area` (the padding box).
pub fn resolve(g: &Gradient, area: Rect) -> Option<ResolvedGradient> {
    let (w, h) = (area.width, area.height);
    if w <= 0.0 || h <= 0.0 {
        return None;
    }
    let (cx0, cy0) = (area.x + w / 2.0, area.y + h / 2.0);
    match g {
        Gradient::Linear {
            direction,
            stops,
            repeating,
        } => {
            let (dx, dy) = match *direction {
                GradientDirection::Angle(a) => {
                    let r = a.to_radians();
                    (r.sin(), -r.cos())
                }
                GradientDirection::Corner(sx, sy) => {
                    let (x, y) = (sx * h, sy * w);
                    let n = (x * x + y * y).sqrt();
                    (x / n, y / n)
                }
            };
            let len = (w * dx).abs() + (h * dy).abs();
            if len <= 1e-6 {
                return None;
            }
            let stops = resolve_stops(stops, len, *repeating, 1.0)?;
            Some(ResolvedGradient {
                geometry: GradientGeometry::Axial {
                    x0: cx0 - dx * len / 2.0,
                    y0: cy0 - dy * len / 2.0,
                    x1: cx0 + dx * len / 2.0,
                    y1: cy0 + dy * len / 2.0,
                },
                stops,
            })
        }
        Gradient::Radial {
            shape,
            size,
            position,
            stops,
            repeating,
        } => {
            let cx = area.x + position.0.resolve(w);
            let cy = area.y + position.1.resolve(h);
            let (l, r) = ((cx - area.x).abs(), (area.x + w - cx).abs());
            let (t, b) = ((cy - area.y).abs(), (area.y + h - cy).abs());
            let corners = [(l, t), (r, t), (l, b), (r, b)];
            let dist = |(x, y): (f32, f32)| (x * x + y * y).sqrt();
            let pick = |far: bool| {
                let mut best = corners[0];
                for c in corners {
                    if (far && dist(c) > dist(best)) || (!far && dist(c) < dist(best)) {
                        best = c;
                    }
                }
                best
            };
            let (rx, ry) = match (shape, size) {
                (_, RadialSize::Explicit(a, b)) => match shape {
                    RadialShape::Circle => {
                        let r = a.resolve(w);
                        (r, r)
                    }
                    RadialShape::Ellipse => (a.resolve(w), b.resolve(h)),
                },
                (RadialShape::Circle, RadialSize::ClosestSide) => {
                    let r = l.min(r).min(t).min(b);
                    (r, r)
                }
                (RadialShape::Circle, RadialSize::FarthestSide) => {
                    let r = l.max(r).max(t).max(b);
                    (r, r)
                }
                (RadialShape::Circle, RadialSize::ClosestCorner) => {
                    let r = dist(pick(false));
                    (r, r)
                }
                (RadialShape::Circle, RadialSize::FarthestCorner) => {
                    let r = dist(pick(true));
                    (r, r)
                }
                (RadialShape::Ellipse, RadialSize::ClosestSide) => (l.min(r), t.min(b)),
                (RadialShape::Ellipse, RadialSize::FarthestSide) => (l.max(r), t.max(b)),
                (RadialShape::Ellipse, RadialSize::ClosestCorner | RadialSize::FarthestCorner) => {
                    // Same aspect ratio as the matching *-side ellipse,
                    // scaled to pass through the corner.
                    let far = matches!(size, RadialSize::FarthestCorner);
                    let (sx, sy) = if far {
                        (l.max(r), t.max(b))
                    } else {
                        (l.min(r), t.min(b))
                    };
                    let (px, py) = pick(far);
                    if sx <= 0.0 || sy <= 0.0 {
                        (0.0, 0.0)
                    } else {
                        let k = ((px / sx).powi(2) + (py / sy).powi(2)).sqrt();
                        (sx * k, sy * k)
                    }
                }
            };
            if rx <= 1e-3 || ry <= 1e-3 {
                // Degenerate ending shape: the last colour fills the box.
                let last = stops.last()?.color;
                return Some(ResolvedGradient {
                    geometry: GradientGeometry::Axial {
                        x0: area.x,
                        y0: area.y,
                        x1: area.x + w,
                        y1: area.y,
                    },
                    stops: vec![(0.0, last), (1.0, last)],
                });
            }
            // A repeating gradient keeps repeating past the ending shape:
            // stretch the shading out to the farthest corner of the box.
            let reach = corners
                .iter()
                .map(|(x, y)| ((x / rx).powi(2) + (y / ry).powi(2)).sqrt())
                .fold(1.0f32, f32::max);
            let scale = if *repeating { reach } else { 1.0 };
            let stops = resolve_stops(stops, rx, *repeating, scale)?;
            Some(ResolvedGradient {
                geometry: GradientGeometry::Radial {
                    cx,
                    cy,
                    rx: rx * scale,
                    ry: ry * scale,
                },
                stops,
            })
        }
    }
}

/// Apply the CSS stop fix-up rules on a gradient line of length `len`, then
/// repeat (if asked) and clip the stops to [0, `span`], and normalise the
/// result to [0, 1].
fn resolve_stops(
    stops: &[ColorStop],
    len: f32,
    repeating: bool,
    span: f32,
) -> Option<Vec<(f32, Color)>> {
    if stops.is_empty() {
        return None;
    }
    let to_frac = |p: GradientLength| p.resolve(len) / len;
    let mut pos: Vec<Option<f32>> = stops.iter().map(|s| s.position.map(to_frac)).collect();
    let n = pos.len();
    if pos[0].is_none() {
        pos[0] = Some(0.0);
    }
    if pos[n - 1].is_none() {
        pos[n - 1] = Some(if n == 1 { 0.0 } else { 1.0 });
    }
    // Positions never go backwards.
    let mut max = f32::MIN;
    for p in pos.iter_mut().flatten() {
        if *p < max {
            *p = max;
        }
        max = *p;
    }
    // Spread unpositioned stops evenly between their positioned neighbours.
    let mut i = 0;
    while i < n {
        if pos[i].is_none() {
            let start = i - 1;
            let mut end = i;
            while pos[end].is_none() {
                end += 1;
            }
            let (a, b) = (pos[start].unwrap(), pos[end].unwrap());
            for (k, p) in pos.iter_mut().enumerate().take(end).skip(i) {
                *p = Some(a + (b - a) * (k - start) as f32 / (end - start) as f32);
            }
            i = end;
        }
        i += 1;
    }
    let mut list: Vec<(f32, Color)> = pos
        .into_iter()
        .zip(stops)
        .map(|(p, s)| (p.unwrap(), s.color))
        .collect();
    if list.len() == 1 {
        list.push(list[0]);
    }

    // Fully transparent stops take the colour of their neighbours, which is
    // what interpolating in premultiplied space amounts to.
    let mut fixed = Vec::with_capacity(list.len() + 2);
    for (k, &(p, c)) in list.iter().enumerate() {
        if c.a < 1e-3 {
            let prev = k.checked_sub(1).map(|j| list[j].1);
            let next = list.get(k + 1).map(|s| s.1);
            let with = |o: Color| Color::new(o.r, o.g, o.b, 0.0);
            match (prev, next) {
                (Some(a), Some(b)) => {
                    fixed.push((p, with(a)));
                    fixed.push((p, with(b)));
                }
                (Some(a), None) => fixed.push((p, with(a))),
                (None, Some(b)) => fixed.push((p, with(b))),
                (None, None) => fixed.push((p, c)),
            }
        } else {
            fixed.push((p, c));
        }
    }
    let mut list = fixed;

    if repeating {
        let (first, last) = (list[0].0, list[list.len() - 1].0);
        let period = last - first;
        if period <= 1e-6 {
            // Zero-length period: CSS paints the average colour; use the last.
            let c = list[list.len() - 1].1;
            return Some(vec![(0.0, c), (1.0, c)]);
        }
        let k0 = ((0.0 - first) / period).floor() as i64;
        let k1 = ((span - first) / period).ceil() as i64;
        if (k1 - k0) as usize * list.len() <= 4096 {
            let base = list.clone();
            list.clear();
            for k in k0..k1 {
                let off = k as f32 * period;
                list.extend(base.iter().map(|&(p, c)| (p + off, c)));
            }
        }
    }

    let mut out = vec![(0.0, color_after(&list, 0.0))];
    out.extend(
        list.iter()
            .filter(|(p, _)| *p > 0.0 && *p < span)
            .map(|&(p, c)| (p / span, c)),
    );
    out.push((1.0, color_before(&list, span)));
    Some(out)
}

fn lerp(a: Color, b: Color, t: f32) -> Color {
    Color::new(
        a.r + (b.r - a.r) * t,
        a.g + (b.g - a.g) * t,
        a.b + (b.b - a.b) * t,
        a.a + (b.a - a.a) * t,
    )
}

/// Colour just after offset `t` (right-continuous at hard stops).
fn color_after(list: &[(f32, Color)], t: f32) -> Color {
    match list.iter().rposition(|(p, _)| *p <= t) {
        None => list[0].1,
        Some(i) if i + 1 == list.len() => list[i].1,
        Some(i) => {
            let ((p0, c0), (p1, c1)) = (list[i], list[i + 1]);
            lerp(c0, c1, (t - p0) / (p1 - p0))
        }
    }
}

/// Colour just before offset `t` (left-continuous at hard stops).
fn color_before(list: &[(f32, Color)], t: f32) -> Color {
    match list.iter().position(|(p, _)| *p >= t) {
        None => list[list.len() - 1].1,
        Some(0) => list[0].1,
        Some(j) => {
            let ((p0, c0), (p1, c1)) = (list[j - 1], list[j]);
            lerp(c0, c1, (t - p0) / (p1 - p0))
        }
    }
}

// ── PDF encoding ──

/// Resources written for one gradient fill.
pub(crate) struct GradientResources {
    pub shading_name: String,
    pub shading_ref: Ref,
    /// ExtGState carrying the alpha soft mask, when a stop is translucent.
    pub mask_state: Option<(String, Ref)>,
}

/// Write the function objects mapping [0, 1] to the stop colours, using
/// `components` to pick the channels (RGB or alpha). Returns the root ref.
fn write_function(
    pdf: &mut Pdf,
    next_ref: &mut dyn FnMut() -> Ref,
    stops: &[(f32, Color)],
    components: fn(Color) -> Vec<f32>,
) -> Ref {
    let segments: Vec<usize> = (0..stops.len() - 1)
        .filter(|&i| stops[i + 1].0 - stops[i].0 > 1e-6)
        .collect();
    let exponential = |pdf: &mut Pdf, r: Ref, c0: Color, c1: Color| {
        let mut f = pdf.exponential_function(r);
        f.domain([0.0, 1.0]);
        f.c0(components(c0));
        f.c1(components(c1));
        f.n(1.0);
        f.finish();
    };
    if segments.len() <= 1 {
        let r = next_ref();
        let (c0, c1) = match segments.first() {
            Some(&i) => (stops[i].1, stops[i + 1].1),
            None => (stops[stops.len() - 1].1, stops[stops.len() - 1].1),
        };
        exponential(pdf, r, c0, c1);
        return r;
    }
    let refs: Vec<Ref> = segments
        .iter()
        .map(|&i| {
            let r = next_ref();
            exponential(pdf, r, stops[i].1, stops[i + 1].1);
            r
        })
        .collect();
    let root = next_ref();
    let mut f = pdf.stitching_function(root);
    f.domain([0.0, 1.0]);
    f.functions(refs.iter().copied());
    f.bounds(segments[1..].iter().map(|&i| stops[i].0));
    f.encode(segments.iter().flat_map(|_| [0.0, 1.0]));
    f.finish();
    root
}

fn write_shading(
    pdf: &mut Pdf,
    r: Ref,
    g: &ResolvedGradient,
    function: Ref,
    gray: bool,
    page_h: f32,
) {
    let mut sh = pdf.function_shading(r);
    match g.geometry {
        GradientGeometry::Axial { x0, y0, x1, y1 } => {
            sh.shading_type(FunctionShadingType::Axial);
            sh.coords([x0, page_h - y0, x1, page_h - y1]);
        }
        GradientGeometry::Radial { cx, cy, rx, .. } => {
            sh.shading_type(FunctionShadingType::Radial);
            sh.coords([cx, page_h - cy, 0.0, cx, page_h - cy, rx]);
        }
    }
    if gray {
        sh.color_space().device_gray();
    } else {
        sh.color_space().device_rgb();
    }
    sh.function(function);
    sh.extend([true, true]);
    sh.finish();
}

/// Matrix squashing the circular PDF shading into the gradient's ellipse.
fn ellipse_matrix(g: &ResolvedGradient, page_h: f32) -> Option<[f32; 6]> {
    match g.geometry {
        GradientGeometry::Radial { cy, rx, ry, .. } if (rx - ry).abs() > 1e-3 => {
            let k = ry / rx;
            let y = page_h - cy;
            Some([1.0, 0.0, 0.0, k, 0.0, y * (1.0 - k)])
        }
        _ => None,
    }
}

/// Write all objects for one gradient painted inside `clip` (page coords).
pub(crate) fn write_gradient(
    pdf: &mut Pdf,
    next_ref: &mut dyn FnMut() -> Ref,
    g: &ResolvedGradient,
    clip: Rect,
    index: usize,
    page_h: f32,
) -> GradientResources {
    let rgb = write_function(pdf, next_ref, &g.stops, |c| vec![c.r, c.g, c.b]);
    let shading_ref = next_ref();
    write_shading(pdf, shading_ref, g, rgb, false, page_h);

    let mask_state = g.has_alpha().then(|| {
        let alpha = write_function(pdf, next_ref, &g.stops, |c| vec![c.a]);
        let alpha_shading = next_ref();
        write_shading(pdf, alpha_shading, g, alpha, true, page_h);

        let mut content = Content::new();
        if let Some(m) = ellipse_matrix(g, page_h) {
            content.transform(m);
        }
        content.shading(Name(b"Sh"));
        let bytes = content.finish();
        let form_ref = next_ref();
        let mut form = pdf.form_xobject(form_ref, &bytes);
        form.bbox(PdfWriterRect::new(
            clip.x,
            page_h - clip.y - clip.height,
            clip.x + clip.width,
            page_h - clip.y,
        ));
        form.group().transparency().color_space().device_gray();
        form.resources().shadings().pair(Name(b"Sh"), alpha_shading);
        form.finish();

        let gs_ref = next_ref();
        let mut gs = pdf.ext_graphics(gs_ref);
        gs.soft_mask().subtype(MaskType::Luminosity).group(form_ref);
        gs.finish();
        (format!("GSg{index}"), gs_ref)
    });

    GradientResources {
        shading_name: format!("Sh{index}"),
        shading_ref,
        mask_state,
    }
}

/// Emit the content-stream operators painting a gradient written by
/// [`write_gradient`]. The caller has already built the clip path.
pub(crate) fn paint_gradient(
    content: &mut Content,
    res: &GradientResources,
    g: &ResolvedGradient,
    page_h: f32,
) {
    if let Some(m) = ellipse_matrix(g, page_h) {
        content.transform(m);
    }
    content.shading(Name(res.shading_name.as_bytes()));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn stop(c: Color, p: Option<f32>) -> ColorStop {
        ColorStop {
            color: c,
            position: p.map(GradientLength::Percent),
        }
    }

    #[test]
    fn fixup_spreads_and_clamps() {
        let red = Color::from_rgb8(255, 0, 0);
        let blue = Color::from_rgb8(0, 0, 255);
        let s = resolve_stops(
            &[
                stop(red, None),
                stop(red, None),
                stop(blue, Some(80.0)),
                stop(blue, Some(40.0)),
            ],
            100.0,
            false,
            1.0,
        )
        .unwrap();
        let offs: Vec<f32> = s.iter().map(|x| x.0).collect();
        // 0, 0.4 (spread), 0.8, 0.8 (clamped), then padded to 1.
        assert_eq!(offs.len(), 5);
        assert!((offs[1] - 0.4).abs() < 1e-5);
        assert!((offs[2] - 0.8).abs() < 1e-5 && (offs[3] - 0.8).abs() < 1e-5);
        assert_eq!(offs[4], 1.0);
    }

    #[test]
    fn repeating_expands_period() {
        let red = Color::from_rgb8(255, 0, 0);
        let blue = Color::from_rgb8(0, 0, 255);
        let s = resolve_stops(
            &[stop(red, Some(0.0)), stop(blue, Some(25.0))],
            100.0,
            true,
            1.0,
        )
        .unwrap();
        // Four periods of two stops.
        assert_eq!(s.len(), 8);
        assert_eq!(s[7].1, blue);
        assert_eq!(s[0].1, red);
    }

    #[test]
    fn transparent_takes_neighbour_colour() {
        let red = Color::from_rgb8(255, 0, 0);
        let s = resolve_stops(
            &[stop(Color::transparent(), None), stop(red, None)],
            100.0,
            false,
            1.0,
        )
        .unwrap();
        assert_eq!((s[0].1.r, s[0].1.a), (1.0, 0.0));
    }

    #[test]
    fn linear_to_right_spans_width() {
        let g = Gradient::Linear {
            direction: GradientDirection::Angle(90.0),
            stops: vec![stop(Color::black(), None), stop(Color::white(), None)],
            repeating: false,
        };
        let r = resolve(&g, Rect::new(10.0, 20.0, 100.0, 50.0)).unwrap();
        match r.geometry {
            GradientGeometry::Axial { x0, y0, x1, y1 } => {
                assert!((x0 - 10.0).abs() < 1e-3 && (x1 - 110.0).abs() < 1e-3);
                assert!((y0 - 45.0).abs() < 1e-3 && (y1 - 45.0).abs() < 1e-3);
            }
            _ => panic!(),
        }
    }

    #[test]
    fn ellipse_farthest_corner() {
        let g = Gradient::Radial {
            shape: RadialShape::Ellipse,
            size: RadialSize::FarthestCorner,
            position: (GradientLength::Percent(50.0), GradientLength::Percent(50.0)),
            stops: vec![stop(Color::black(), None), stop(Color::white(), None)],
            repeating: false,
        };
        let r = resolve(&g, Rect::new(0.0, 0.0, 200.0, 100.0)).unwrap();
        match r.geometry {
            GradientGeometry::Radial { rx, ry, .. } => {
                assert!((rx - 100.0 * 2f32.sqrt()).abs() < 1e-2);
                assert!((ry - 50.0 * 2f32.sqrt()).abs() < 1e-2);
            }
            _ => panic!(),
        }
    }
}
