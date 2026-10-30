//! CSS gradient images (`linear-gradient()`, `radial-gradient()` and their
//! `repeating-` variants), as parsed from `background` / `background-image`.

use crate::Color;

/// A position or a size expressed either in points or as a percentage of a
/// reference length (the gradient line / ray for colour stops, the box for
/// positions and radii).
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientLength {
    Pt(f32),
    Percent(f32),
}

impl GradientLength {
    pub fn resolve(self, reference: f32) -> f32 {
        match self {
            GradientLength::Pt(v) => v,
            GradientLength::Percent(p) => p / 100.0 * reference,
        }
    }
}

/// One colour stop. A stop without a position is placed by the CSS fix-up
/// rules when the gradient is resolved against a box.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ColorStop {
    pub color: Color,
    pub position: Option<GradientLength>,
}

/// Direction of a linear gradient.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GradientDirection {
    /// CSS angle in degrees: 0 = towards the top, 90 = towards the right.
    Angle(f32),
    /// `to <corner>`: horizontal sign (+1 right, -1 left) and vertical sign
    /// (+1 bottom, -1 top). The real angle depends on the box's aspect ratio.
    Corner(f32, f32),
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialShape {
    Circle,
    Ellipse,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum RadialSize {
    ClosestSide,
    FarthestSide,
    ClosestCorner,
    FarthestCorner,
    /// Explicit radii (a circle uses the first value only).
    Explicit(GradientLength, GradientLength),
}

#[derive(Debug, Clone, PartialEq)]
pub enum Gradient {
    Linear {
        direction: GradientDirection,
        stops: Vec<ColorStop>,
        repeating: bool,
    },
    Radial {
        shape: RadialShape,
        size: RadialSize,
        /// Centre, relative to the box (default `center`).
        position: (GradientLength, GradientLength),
        stops: Vec<ColorStop>,
        repeating: bool,
    },
}
