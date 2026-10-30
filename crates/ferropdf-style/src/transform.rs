//! Parsing of `transform`, `translate`, `rotate`, `scale` and
//! `transform-origin`.
//!
//! 3D functions are flattened onto the page (orthographic projection):
//! `rotateX(a)` squashes vertically by cos(a), `rotateY(a)` horizontally,
//! `translateZ`, `scaleZ` and `perspective` have no visible 2D effect.

use crate::gradient::{parse_angle, parse_length_pct, parse_position, split_top_level, words};
use ferropdf_core::{GradientLength, TransformFunction};

const PX: f32 = 0.75;

fn args(inner: &str) -> Vec<&str> {
    split_top_level(inner, |c| c == ',' || c.is_whitespace())
}

fn number(s: &str) -> Option<f32> {
    match s.strip_suffix('%') {
        Some(p) => p.trim().parse::<f32>().ok().map(|v| v / 100.0),
        None => s.parse::<f32>().ok(),
    }
}

fn zero() -> GradientLength {
    GradientLength::Pt(0.0)
}

/// Parse a `transform` value. `Some(vec![])` for `none`, `None` when invalid.
pub(crate) fn parse_transform(raw: &str) -> Option<Vec<TransformFunction>> {
    let raw = raw.trim();
    if raw.eq_ignore_ascii_case("none") {
        return Some(Vec::new());
    }
    let mut out = Vec::new();
    for f in words(raw) {
        let open = f.find('(')?;
        let name = f[..open].trim().to_ascii_lowercase();
        let inner = f[open + 1..].strip_suffix(')')?;
        let a = args(inner);
        let len = |i: usize| a.get(i).and_then(|s| parse_length_pct(s));
        let ang = |i: usize| a.get(i).and_then(|s| parse_angle(s));
        let num = |i: usize| a.get(i).and_then(|s| number(s));
        use TransformFunction as T;
        let func = match (name.as_str(), a.len()) {
            ("translate", 1) => T::Translate(len(0)?, zero()),
            ("translate", 2) | ("translate3d", 3) => T::Translate(len(0)?, len(1)?),
            ("translatex", 1) => T::Translate(len(0)?, zero()),
            ("translatey", 1) => T::Translate(zero(), len(0)?),
            ("translatez", 1) | ("perspective", 1) => continue,
            ("scale", 1) => T::Scale(num(0)?, num(0)?),
            ("scale", 2) | ("scale3d", 3) => T::Scale(num(0)?, num(1)?),
            ("scalex", 1) => T::Scale(num(0)?, 1.0),
            ("scaley", 1) => T::Scale(1.0, num(0)?),
            ("scalez", 1) => continue,
            ("rotate" | "rotatez", 1) => T::Rotate(ang(0)?),
            ("rotatex", 1) => T::Scale(1.0, ang(0)?.to_radians().cos()),
            ("rotatey", 1) => T::Scale(ang(0)?.to_radians().cos(), 1.0),
            ("skew", 1) => T::Skew(ang(0)?, 0.0),
            ("skew", 2) => T::Skew(ang(0)?, ang(1)?),
            ("skewx", 1) => T::Skew(ang(0)?, 0.0),
            ("skewy", 1) => T::Skew(0.0, ang(0)?),
            ("matrix", 6) => {
                let v: Option<Vec<f32>> = a.iter().map(|s| s.parse::<f32>().ok()).collect();
                let v = v?;
                T::Matrix([v[0], v[1], v[2], v[3], v[4] * PX, v[5] * PX])
            }
            ("matrix3d", 16) => {
                let v: Option<Vec<f32>> = a.iter().map(|s| s.parse::<f32>().ok()).collect();
                let v = v?;
                T::Matrix([v[0], v[1], v[4], v[5], v[12] * PX, v[13] * PX])
            }
            _ => return None,
        };
        out.push(func);
    }
    Some(out)
}

/// The individual `translate` property: one to three lengths.
pub(crate) fn parse_translate(raw: &str) -> Option<Option<TransformFunction>> {
    let w = words(raw.trim());
    match w.as_slice() {
        ["none"] => Some(None),
        [x] => Some(Some(TransformFunction::Translate(
            parse_length_pct(x)?,
            zero(),
        ))),
        [x, y] | [x, y, _] => Some(Some(TransformFunction::Translate(
            parse_length_pct(x)?,
            parse_length_pct(y)?,
        ))),
        _ => None,
    }
}

/// The individual `rotate` property: an angle, optionally after the `z` axis.
pub(crate) fn parse_rotate(raw: &str) -> Option<Option<TransformFunction>> {
    let w = words(raw.trim());
    match w.as_slice() {
        ["none"] => Some(None),
        [a] | ["z", a] | [a, "z"] => Some(Some(TransformFunction::Rotate(parse_angle(a)?))),
        _ => None,
    }
}

/// The individual `scale` property: one to three numbers or percentages.
pub(crate) fn parse_scale(raw: &str) -> Option<Option<TransformFunction>> {
    let w = words(raw.trim());
    match w.as_slice() {
        ["none"] => Some(None),
        [s] => {
            let s = number(s)?;
            Some(Some(TransformFunction::Scale(s, s)))
        }
        [x, y] | [x, y, _] => Some(Some(TransformFunction::Scale(number(x)?, number(y)?))),
        _ => None,
    }
}

/// `transform-origin`: one or two values (a third, z, value is ignored).
pub(crate) fn parse_transform_origin(raw: &str) -> Option<(GradientLength, GradientLength)> {
    let lower = raw.trim().to_ascii_lowercase();
    let w = words(&lower);
    match w.as_slice() {
        [_] | [_, _] => parse_position(&w),
        [a, b, _] => parse_position(&[a, b]),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use TransformFunction as T;

    #[test]
    fn transform_lists() {
        assert_eq!(parse_transform("none"), Some(vec![]));
        assert_eq!(
            parse_transform("translate(-50%, -50%) rotate(45deg)"),
            Some(vec![
                T::Translate(
                    GradientLength::Percent(-50.0),
                    GradientLength::Percent(-50.0)
                ),
                T::Rotate(45.0),
            ])
        );
        assert_eq!(
            parse_transform("scale(2) translateX(10px)"),
            Some(vec![
                T::Scale(2.0, 2.0),
                T::Translate(GradientLength::Pt(7.5), GradientLength::Pt(0.0)),
            ])
        );
        assert_eq!(
            parse_transform("rotate(0.5turn)"),
            Some(vec![T::Rotate(180.0)])
        );
        assert!(parse_transform("wobble(3)").is_none());
        assert_eq!(
            parse_transform("matrix(1, 0, 0, 1, 20, 40)"),
            Some(vec![T::Matrix([1.0, 0.0, 0.0, 1.0, 15.0, 30.0])])
        );
    }

    #[test]
    fn individual_properties_and_origin() {
        assert_eq!(parse_rotate("90deg"), Some(Some(T::Rotate(90.0))));
        assert_eq!(parse_scale("150%"), Some(Some(T::Scale(1.5, 1.5))));
        assert_eq!(parse_translate("none"), Some(None));
        assert_eq!(
            parse_transform_origin("top left"),
            Some((GradientLength::Percent(0.0), GradientLength::Percent(0.0)))
        );
        assert_eq!(
            parse_transform_origin("bottom"),
            Some((
                GradientLength::Percent(50.0),
                GradientLength::Percent(100.0)
            ))
        );
    }
}
