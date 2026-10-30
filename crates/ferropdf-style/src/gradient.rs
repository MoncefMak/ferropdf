//! Parsing of CSS gradient functions and the `background` shorthand.

use crate::cascade::{parse_color, parse_length_to_pt};
use ferropdf_core::{
    Color, ColorStop, Gradient, GradientDirection, GradientLength, RadialShape, RadialSize,
};

/// Split `s` on `sep` at parenthesis depth 0. Empty pieces are dropped.
pub(crate) fn split_top_level(s: &str, sep: impl Fn(char) -> bool) -> Vec<&str> {
    let mut out = Vec::new();
    let mut depth = 0i32;
    let mut start = 0;
    for (i, ch) in s.char_indices() {
        match ch {
            '(' => depth += 1,
            ')' => depth -= 1,
            c if depth == 0 && sep(c) => {
                let piece = s[start..i].trim();
                if !piece.is_empty() {
                    out.push(piece);
                }
                start = i + c.len_utf8();
            }
            _ => {}
        }
    }
    let piece = s[start..].trim();
    if !piece.is_empty() {
        out.push(piece);
    }
    out
}

pub(crate) fn words(s: &str) -> Vec<&str> {
    split_top_level(s, char::is_whitespace)
}

/// Parsed `background` shorthand: the colour of the final layer and the
/// gradient layers (first = topmost). `None` when the value is invalid.
pub(crate) struct Background {
    pub color: Color,
    pub gradients: Vec<Gradient>,
}

pub(crate) fn parse_background(raw: &str) -> Option<Background> {
    let mut color = Color::transparent();
    let mut gradients = Vec::new();
    let layers = split_top_level(raw, |c| c == ',');
    let n = layers.len();
    for (i, layer) in layers.into_iter().enumerate() {
        for word in words(layer) {
            if is_gradient_function(word) {
                gradients.push(parse_gradient(word)?);
            } else if i + 1 == n {
                if let Some(c) = parse_color(word) {
                    color = c;
                }
            }
        }
    }
    Some(Background { color, gradients })
}

/// `background-image`: a comma-separated list of images; only gradients are
/// kept (`url()` images and `none` produce no layer).
pub(crate) fn parse_background_image(raw: &str) -> Option<Vec<Gradient>> {
    let mut out = Vec::new();
    for layer in split_top_level(raw, |c| c == ',') {
        if is_gradient_function(layer) {
            out.push(parse_gradient(layer)?);
        }
    }
    Some(out)
}

fn is_gradient_function(s: &str) -> bool {
    let l = s.to_ascii_lowercase();
    l.starts_with("linear-gradient(")
        || l.starts_with("radial-gradient(")
        || l.starts_with("repeating-linear-gradient(")
        || l.starts_with("repeating-radial-gradient(")
}

pub(crate) fn parse_gradient(s: &str) -> Option<Gradient> {
    let open = s.find('(')?;
    let name = s[..open].trim().to_ascii_lowercase();
    let inner = s[open + 1..].trim_end().strip_suffix(')')?;
    let args = split_top_level(inner, |c| c == ',');
    let (repeating, kind) = match name.strip_prefix("repeating-") {
        Some(rest) => (true, rest.to_string()),
        None => (false, name),
    };
    match kind.as_str() {
        "linear-gradient" => parse_linear(&args, repeating),
        "radial-gradient" => parse_radial(&args, repeating),
        _ => None,
    }
}

fn parse_linear(args: &[&str], repeating: bool) -> Option<Gradient> {
    let (first, rest) = args.split_first()?;
    let (direction, stop_args) = match parse_direction(first) {
        Some(d) => (d, rest),
        None => (GradientDirection::Angle(180.0), args),
    };
    Some(Gradient::Linear {
        direction,
        stops: parse_stops(stop_args)?,
        repeating,
    })
}

fn parse_direction(s: &str) -> Option<GradientDirection> {
    let w = words(&s.to_ascii_lowercase())
        .into_iter()
        .map(str::to_string)
        .collect::<Vec<_>>();
    if w.first().map(String::as_str) == Some("to") {
        let (mut h, mut v) = (0.0f32, 0.0f32);
        for k in &w[1..] {
            match k.as_str() {
                "left" => h = -1.0,
                "right" => h = 1.0,
                "top" => v = -1.0,
                "bottom" => v = 1.0,
                _ => return None,
            }
        }
        return match (h != 0.0, v != 0.0) {
            (true, true) => Some(GradientDirection::Corner(h, v)),
            (true, false) => Some(GradientDirection::Angle(if h > 0.0 { 90.0 } else { 270.0 })),
            (false, true) => Some(GradientDirection::Angle(if v > 0.0 { 180.0 } else { 0.0 })),
            _ => None,
        };
    }
    if w.len() == 1 {
        return parse_angle(&w[0]).map(GradientDirection::Angle);
    }
    None
}

pub(crate) fn parse_angle(s: &str) -> Option<f32> {
    let num = |suffix: &str| s.strip_suffix(suffix)?.parse::<f32>().ok();
    if let Some(v) = num("deg") {
        Some(v)
    } else if let Some(v) = num("grad") {
        Some(v * 0.9)
    } else if let Some(v) = num("rad") {
        Some(v.to_degrees())
    } else if let Some(v) = num("turn") {
        Some(v * 360.0)
    } else if s == "0" {
        Some(0.0)
    } else {
        None
    }
}

pub(crate) fn parse_length_pct(s: &str) -> Option<GradientLength> {
    if let Some(p) = s.strip_suffix('%') {
        return p.trim().parse::<f32>().ok().map(GradientLength::Percent);
    }
    // A bare non-zero number is not a length here (it would be a colour
    // component or garbage); `parse_length_to_pt` would accept it as px.
    if s.parse::<f32>().is_ok() && s.parse::<f32>().ok() != Some(0.0) {
        return None;
    }
    parse_length_to_pt(s).map(GradientLength::Pt)
}

fn parse_stops(args: &[&str]) -> Option<Vec<ColorStop>> {
    let mut stops = Vec::new();
    for arg in args {
        let w = words(arg);
        match w.as_slice() {
            // A lone length is a colour-interpolation hint: not supported,
            // the midpoint stays halfway.
            [single] if parse_length_pct(single).is_some() => {}
            [color, positions @ ..] if positions.len() <= 2 => {
                let (color, positions): (Color, Vec<&str>) = match parse_color(color) {
                    Some(c) => (c, positions.to_vec()),
                    // Position written before the colour.
                    None if positions.len() == 1 => (parse_color(positions[0])?, vec![*color]),
                    None => return None,
                };
                if positions.is_empty() {
                    stops.push(ColorStop {
                        color,
                        position: None,
                    });
                }
                for p in positions {
                    stops.push(ColorStop {
                        color,
                        position: Some(parse_length_pct(p)?),
                    });
                }
            }
            _ => return None,
        }
    }
    (!stops.is_empty()).then_some(stops)
}

fn parse_radial(args: &[&str], repeating: bool) -> Option<Gradient> {
    let (first, rest) = args.split_first()?;
    let lower = first.to_ascii_lowercase();
    let w: Vec<&str> = words(&lower);
    let is_config = w.iter().any(|t| {
        matches!(
            *t,
            "circle"
                | "ellipse"
                | "closest-side"
                | "farthest-side"
                | "closest-corner"
                | "farthest-corner"
                | "at"
        )
    }) || w.first().is_some_and(|t| parse_length_pct(t).is_some());

    let mut shape = None;
    let mut size = None;
    let mut position = (GradientLength::Percent(50.0), GradientLength::Percent(50.0));
    let stop_args = if is_config {
        let at = w.iter().position(|t| *t == "at");
        let (head, pos) = match at {
            Some(i) => (&w[..i], Some(&w[i + 1..])),
            None => (&w[..], None),
        };
        let mut lengths = Vec::new();
        for t in head {
            match *t {
                "circle" => shape = Some(RadialShape::Circle),
                "ellipse" => shape = Some(RadialShape::Ellipse),
                "closest-side" => size = Some(RadialSize::ClosestSide),
                "farthest-side" => size = Some(RadialSize::FarthestSide),
                "closest-corner" => size = Some(RadialSize::ClosestCorner),
                "farthest-corner" => size = Some(RadialSize::FarthestCorner),
                other => lengths.push(parse_length_pct(other)?),
            }
        }
        match lengths.as_slice() {
            [] => {}
            [r] => {
                size = Some(RadialSize::Explicit(*r, *r));
                shape.get_or_insert(RadialShape::Circle);
            }
            [rx, ry] => {
                size = Some(RadialSize::Explicit(*rx, *ry));
                shape.get_or_insert(RadialShape::Ellipse);
            }
            _ => return None,
        }
        if let Some(pos) = pos {
            position = parse_position(pos)?;
        }
        rest
    } else {
        args
    };
    Some(Gradient::Radial {
        shape: shape.unwrap_or(RadialShape::Ellipse),
        size: size.unwrap_or(RadialSize::FarthestCorner),
        position,
        stops: parse_stops(stop_args)?,
        repeating,
    })
}

/// `<position>` with one or two values (keywords, lengths, percentages).
pub(crate) fn parse_position(w: &[&str]) -> Option<(GradientLength, GradientLength)> {
    let kw = |t: &str| -> Option<(GradientLength, Option<bool>)> {
        // (value, Some(true) = horizontal-only, Some(false) = vertical-only)
        let p = GradientLength::Percent;
        Some(match t {
            "left" => (p(0.0), Some(true)),
            "right" => (p(100.0), Some(true)),
            "top" => (p(0.0), Some(false)),
            "bottom" => (p(100.0), Some(false)),
            "center" => (p(50.0), None),
            other => (parse_length_pct(other)?, None),
        })
    };
    match w {
        [a] => {
            let (v, axis) = kw(a)?;
            let c = GradientLength::Percent(50.0);
            Some(if axis == Some(false) { (c, v) } else { (v, c) })
        }
        [a, b] => {
            let (va, axa) = kw(a)?;
            let (vb, axb) = kw(b)?;
            if axa == Some(false) || axb == Some(true) {
                Some((vb, va))
            } else {
                Some((va, vb))
            }
        }
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn linear_defaults_and_directions() {
        let g = parse_gradient("linear-gradient(red, blue)").unwrap();
        match g {
            Gradient::Linear {
                direction, stops, ..
            } => {
                assert_eq!(direction, GradientDirection::Angle(180.0));
                assert_eq!(stops.len(), 2);
            }
            _ => panic!(),
        }
        let g = parse_gradient("linear-gradient(to top right, red 10%, rgba(0, 0, 0, .5) 20px)")
            .unwrap();
        match g {
            Gradient::Linear {
                direction, stops, ..
            } => {
                assert_eq!(direction, GradientDirection::Corner(1.0, -1.0));
                assert_eq!(stops[0].position, Some(GradientLength::Percent(10.0)));
                assert_eq!(stops[1].position, Some(GradientLength::Pt(15.0)));
                assert!((stops[1].color.a - 0.5).abs() < 1e-3);
            }
            _ => panic!(),
        }
        assert!(matches!(
            parse_gradient("linear-gradient(0.25turn, red, blue)"),
            Some(Gradient::Linear {
                direction: GradientDirection::Angle(a),
                ..
            }) if (a - 90.0).abs() < 1e-3
        ));
    }

    #[test]
    fn two_position_stops_and_repeating() {
        match parse_gradient("repeating-linear-gradient(45deg, red 0 10px, blue 10px 20px)") {
            Some(Gradient::Linear {
                stops, repeating, ..
            }) => {
                assert!(repeating);
                assert_eq!(stops.len(), 4);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn radial_config() {
        match parse_gradient("radial-gradient(circle at top left, red, blue)") {
            Some(Gradient::Radial {
                shape, position, ..
            }) => {
                assert_eq!(shape, RadialShape::Circle);
                assert_eq!(
                    position,
                    (GradientLength::Percent(0.0), GradientLength::Percent(0.0))
                );
            }
            other => panic!("{other:?}"),
        }
        match parse_gradient("radial-gradient(red, blue)") {
            Some(Gradient::Radial { shape, size, .. }) => {
                assert_eq!(shape, RadialShape::Ellipse);
                assert_eq!(size, RadialSize::FarthestCorner);
            }
            other => panic!("{other:?}"),
        }
    }

    #[test]
    fn background_shorthand() {
        let b = parse_background("#fff linear-gradient(red, blue)").unwrap();
        assert_eq!(b.gradients.len(), 1);
        assert_eq!(b.color, Color::white());
        assert!(parse_background("linear-gradient(nonsense)").is_none());
        let b = parse_background("red").unwrap();
        assert!(b.gradients.is_empty());
    }
}
