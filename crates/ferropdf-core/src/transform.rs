//! CSS 2D transforms (`transform`, `translate`, `rotate`, `scale`,
//! `transform-origin`).
//!
//! Matrices use the CSS / SVG convention `[a, b, c, d, e, f]`:
//! `x' = a·x + c·y + e`, `y' = b·x + d·y + f`, in a y-down space.

use crate::GradientLength;

pub type Matrix = [f32; 6];

pub const IDENTITY: Matrix = [1.0, 0.0, 0.0, 1.0, 0.0, 0.0];

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum TransformFunction {
    /// Percentages refer to the border box width / height.
    Translate(GradientLength, GradientLength),
    Scale(f32, f32),
    /// Degrees, clockwise on the page.
    Rotate(f32),
    /// Skew angles in degrees along x and y.
    Skew(f32, f32),
    /// `matrix(a, b, c, d, e, f)` with e / f already in pt.
    Matrix(Matrix),
}

impl TransformFunction {
    /// Matrix of this function for a border box of `w` × `h` pt.
    pub fn to_matrix(&self, w: f32, h: f32) -> Matrix {
        match *self {
            TransformFunction::Translate(x, y) => [1.0, 0.0, 0.0, 1.0, x.resolve(w), y.resolve(h)],
            TransformFunction::Scale(sx, sy) => [sx, 0.0, 0.0, sy, 0.0, 0.0],
            TransformFunction::Rotate(deg) => {
                let (s, c) = deg.to_radians().sin_cos();
                [c, s, -s, c, 0.0, 0.0]
            }
            TransformFunction::Skew(ax, ay) => [
                1.0,
                ay.to_radians().tan(),
                ax.to_radians().tan(),
                1.0,
                0.0,
                0.0,
            ],
            TransformFunction::Matrix(m) => m,
        }
    }
}

/// `a · b`: the transform applying `b` first, then `a`.
pub fn multiply(a: Matrix, b: Matrix) -> Matrix {
    [
        a[0] * b[0] + a[2] * b[1],
        a[1] * b[0] + a[3] * b[1],
        a[0] * b[2] + a[2] * b[3],
        a[1] * b[2] + a[3] * b[3],
        a[0] * b[4] + a[2] * b[5] + a[4],
        a[1] * b[4] + a[3] * b[5] + a[5],
    ]
}

/// The full matrix of a box: individual `translate`, `rotate`, `scale`
/// properties, then the `transform` list, all around the origin
/// `(ox, oy)` (absolute page coordinates). `None` when there is nothing to
/// apply.
pub fn box_matrix(
    individual: &[Option<TransformFunction>; 3],
    list: &[TransformFunction],
    w: f32,
    h: f32,
    ox: f32,
    oy: f32,
) -> Option<Matrix> {
    if list.is_empty() && individual.iter().all(Option::is_none) {
        return None;
    }
    let mut m = IDENTITY;
    for f in individual.iter().flatten().chain(list) {
        m = multiply(m, f.to_matrix(w, h));
    }
    let to_origin = [1.0, 0.0, 0.0, 1.0, ox, oy];
    let from_origin = [1.0, 0.0, 0.0, 1.0, -ox, -oy];
    Some(multiply(multiply(to_origin, m), from_origin))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn apply(m: Matrix, x: f32, y: f32) -> (f32, f32) {
        (m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5])
    }

    #[test]
    fn rotate_around_origin() {
        // 90deg clockwise around (10, 10): (20, 10) → (10, 20).
        let m = box_matrix(
            &[None, None, None],
            &[TransformFunction::Rotate(90.0)],
            20.0,
            20.0,
            10.0,
            10.0,
        )
        .unwrap();
        let (x, y) = apply(m, 20.0, 10.0);
        assert!(
            (x - 10.0).abs() < 1e-4 && (y - 20.0).abs() < 1e-4,
            "{x},{y}"
        );
    }

    #[test]
    fn list_applies_right_to_left() {
        // translate(10px) then scale(2): a point at 0 ends at 10 (scale first).
        let m = box_matrix(
            &[None, None, None],
            &[
                TransformFunction::Translate(GradientLength::Pt(10.0), GradientLength::Pt(0.0)),
                TransformFunction::Scale(2.0, 2.0),
            ],
            0.0,
            0.0,
            0.0,
            0.0,
        )
        .unwrap();
        assert_eq!(apply(m, 0.0, 0.0), (10.0, 0.0));
        assert_eq!(apply(m, 1.0, 0.0), (12.0, 0.0));
    }

    #[test]
    fn translate_percent_uses_box_size() {
        let f = TransformFunction::Translate(
            GradientLength::Percent(-50.0),
            GradientLength::Percent(-50.0),
        );
        assert_eq!(f.to_matrix(100.0, 40.0)[4..], [-50.0, -20.0]);
    }
}
