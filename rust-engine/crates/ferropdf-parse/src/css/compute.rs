//! Resolve raw CSS values to absolute computed values.

use ferropdf_core::{Color, Size};
use super::values::CssLength;

/// Context available when computing values for one element.
pub struct ComputeCtx {
    /// Current element's font-size in px.
    pub font_size_px:    f32,
    /// Root element's font-size in px.
    pub root_font_px:    f32,
    /// Parent element's font-size (for `em`).
    pub parent_font_px:  f32,
    /// Parent element's inline dimension (for %).
    pub parent_width_px: f32,
    /// Viewport dimensions.
    pub viewport:        Size,
    /// Resolved `currentColor` (= inherited color).
    pub current_color:   Color,
}

impl Default for ComputeCtx {
    fn default() -> Self {
        Self {
            font_size_px:    16.0,
            root_font_px:    16.0,
            parent_font_px:  16.0,
            parent_width_px: 0.0,
            viewport:        Size { width: 793.7, height: 1122.5 }, // A4 at 96dpi
            current_color:   Color::BLACK,
        }
    }
}

/// Convert a `CssLength` to absolute pixels.
pub fn resolve_length(length: CssLength, ctx: &ComputeCtx) -> f32 {
    match length {
        CssLength::Px(v)      => v,
        CssLength::Mm(v)      => v * 96.0 / 25.4,
        CssLength::Cm(v)      => v * 96.0 / 2.54,
        CssLength::Pt(v)      => v * 96.0 / 72.0,
        CssLength::Em(v)      => v * ctx.font_size_px,
        CssLength::Rem(v)     => v * ctx.root_font_px,
        CssLength::Percent(v) => v / 100.0 * ctx.parent_width_px,
        CssLength::Vw(v)      => v / 100.0 * ctx.viewport.width,
        CssLength::Vh(v)      => v / 100.0 * ctx.viewport.height,
        CssLength::Zero       => 0.0,
        CssLength::Auto       => 0.0,
    }
}

/// Resolve a raw length string (e.g. "12px", "1.5em") to pixels.
///
/// Returns `None` if the string is not a valid length.
pub fn parse_length_str(s: &str, ctx: &ComputeCtx) -> Option<f32> {
    let s = s.trim();
    if s == "0" || s == "0px" { return Some(0.0); }
    if s == "auto" { return None; }  // caller must handle auto specially

    let (num_str, unit) = split_num_unit(s)?;
    let v: f32 = num_str.parse().ok()?;

    let len = match unit {
        "px"   => CssLength::Px(v),
        "mm"   => CssLength::Mm(v),
        "cm"   => CssLength::Cm(v),
        "pt"   => CssLength::Pt(v),
        "em"   => CssLength::Em(v),
        "rem"  => CssLength::Rem(v),
        "%"    => CssLength::Percent(v),
        "vw"   => CssLength::Vw(v),
        "vh"   => CssLength::Vh(v),
        _      => return None,
    };

    Some(resolve_length(len, ctx))
}

fn split_num_unit(s: &str) -> Option<(&str, &str)> {
    let unit_start = s.find(|c: char| c.is_alphabetic() || c == '%')?;
    Some((&s[..unit_start], &s[unit_start..]))
}

/// Parse a font-size keyword or value.
pub fn resolve_font_size(s: &str, parent_font_px: f32, root_font_px: f32) -> f32 {
    match s {
        "xx-small"  => 9.0,
        "x-small"   => 10.0,
        "small"     => 13.0,
        "medium"    => 16.0,
        "large"     => 18.0,
        "x-large"   => 24.0,
        "xx-large"  => 32.0,
        "smaller"   => parent_font_px * 0.83,
        "larger"    => parent_font_px * 1.2,
        _ => {
            let ctx = ComputeCtx {
                font_size_px: parent_font_px,
                parent_font_px,
                root_font_px,
                parent_width_px: 0.0,
                ..Default::default()
            };
            parse_length_str(s, &ctx).unwrap_or(parent_font_px)
        }
    }
}

/// Parse a CSS color string into `Color`.
pub fn parse_color(s: &str) -> Option<Color> {
    let s = s.trim();

    // Named colors
    if let Some(c) = named_color(s) { return Some(c); }

    // #rgb, #rgba, #rrggbb, #rrggbbaa
    if let Some(stripped) = s.strip_prefix('#') {
        return parse_hex_color(stripped);
    }

    // rgb() / rgba()
    if let Some(inner) = s.strip_prefix("rgb(").and_then(|s| s.strip_suffix(')')) {
        return parse_rgb_args(inner, false);
    }
    if let Some(inner) = s.strip_prefix("rgba(").and_then(|s| s.strip_suffix(')')) {
        return parse_rgb_args(inner, true);
    }

    None
}

fn parse_hex_color(hex: &str) -> Option<Color> {
    let nibbles: Vec<u8> = hex.chars().map(|c| c.to_digit(16).map(|n| n as u8)).collect::<Option<_>>()?;
    match nibbles.len() {
        3 => Some(Color::from_rgb8(nibbles[0] * 17, nibbles[1] * 17, nibbles[2] * 17)),
        4 => Some(Color::from_rgba8(nibbles[0]*17, nibbles[1]*17, nibbles[2]*17, nibbles[3]*17)),
        6 => Some(Color::from_rgb8(nibbles[0]*16+nibbles[1], nibbles[2]*16+nibbles[3], nibbles[4]*16+nibbles[5])),
        8 => Some(Color::from_rgba8(
            nibbles[0]*16+nibbles[1], nibbles[2]*16+nibbles[3],
            nibbles[4]*16+nibbles[5], nibbles[6]*16+nibbles[7],
        )),
        _ => None,
    }
}

fn parse_rgb_args(args: &str, has_alpha: bool) -> Option<Color> {
    let parts: Vec<&str> = args.split(',').collect();
    if has_alpha && parts.len() != 4 { return None; }
    if !has_alpha && parts.len() != 3 { return None; }

    let parse_channel = |s: &str| -> Option<u8> {
        let s = s.trim();
        if let Some(pct) = s.strip_suffix('%') {
            let v: f32 = pct.trim().parse().ok()?;
            Some((v / 100.0 * 255.0).round() as u8)
        } else {
            let v: f32 = s.parse().ok()?;
            Some(v.round() as u8)
        }
    };

    let r = parse_channel(parts[0])?;
    let g = parse_channel(parts[1])?;
    let b = parse_channel(parts[2])?;
    let a = if has_alpha {
        let v: f32 = parts[3].trim().parse().ok()?;
        (v * 255.0).round() as u8
    } else { 255 };

    Some(Color::from_rgba8(r, g, b, a))
}

fn named_color(name: &str) -> Option<Color> {
    let c = |r,g,b| Color::from_rgb8(r,g,b);
    match name.to_ascii_lowercase().as_str() {
        "black"       => Some(c(  0,   0,   0)),
        "silver"      => Some(c(192, 192, 192)),
        "gray" | "grey" => Some(c(128, 128, 128)),
        "white"       => Some(c(255, 255, 255)),
        "maroon"      => Some(c(128,   0,   0)),
        "red"         => Some(c(255,   0,   0)),
        "purple"      => Some(c(128,   0, 128)),
        "fuchsia"
        | "magenta"   => Some(c(255,   0, 255)),
        "green"       => Some(c(  0, 128,   0)),
        "lime"        => Some(c(  0, 255,   0)),
        "olive"       => Some(c(128, 128,   0)),
        "yellow"      => Some(c(255, 255,   0)),
        "navy"        => Some(c(  0,   0, 128)),
        "blue"        => Some(c(  0,   0, 255)),
        "teal"        => Some(c(  0, 128, 128)),
        "aqua" | "cyan" => Some(c( 0, 255, 255)),
        "orange"      => Some(c(255, 165,   0)),
        "coral"       => Some(c(255,  99,  71)),
        "tomato"      => Some(c(255,  99,  71)),
        "crimson"     => Some(c(220,  20,  60)),
        "indianred"   => Some(c(205,  92,  92)),
        "lightcoral"  => Some(c(240, 128, 128)),
        "darkred"     => Some(c(139,   0,   0)),
        "pink"        => Some(c(255, 192, 203)),
        "hotpink"     => Some(c(255, 105, 180)),
        "deeppink"    => Some(c(255,  20, 147)),
        "salmon"      => Some(c(250, 128, 114)),
        "lightsalmon" => Some(c(255, 160, 122)),
        "gold"        => Some(c(255, 215,   0)),
        "goldenrod"   => Some(c(218, 165,  32)),
        "khaki"       => Some(c(240, 230, 140)),
        "tan"         => Some(c(210, 180, 140)),
        "bisque"      => Some(c(255, 228, 196)),
        "wheat"       => Some(c(245, 222, 179)),
        "greenyellow"   => Some(c(173, 255,  47)),
        "yellowgreen"   => Some(c(154, 205,  50)),
        "chartreuse"    => Some(c(127, 255,   0)),
        "lawngreen"     => Some(c(124, 252,   0)),
        "limegreen"     => Some(c( 50, 205,  50)),
        "palegreen"     => Some(c(152, 251, 152)),
        "lightgreen"    => Some(c(144, 238, 144)),
        "mediumseagreen"  => Some(c( 60, 179, 113)),
        "seagreen"        => Some(c( 46, 139,  87)),
        "darkgreen"       => Some(c(  0, 100,   0)),
        "forestgreen"     => Some(c( 34, 139,  34)),
        "olivedrab"       => Some(c(107, 142,  35)),
        "darkkhaki"       => Some(c(189, 183, 107)),
        "skyblue"         => Some(c(135, 206, 235)),
        "lightskyblue"    => Some(c(135, 206, 250)),
        "steelblue"       => Some(c( 70, 130, 180)),
        "dodgerblue"      => Some(c( 30, 144, 255)),
        "cornflowerblue"  => Some(c(100, 149, 237)),
        "royalblue"       => Some(c( 65, 105, 225)),
        "mediumblue"      => Some(c(  0,   0, 205)),
        "darkblue"        => Some(c(  0,   0, 139)),
        "midnightblue"    => Some(c( 25,  25, 112)),
        "cadetblue"       => Some(c( 95, 158, 160)),
        "powderblue"      => Some(c(176, 224, 230)),
        "lightblue"       => Some(c(173, 216, 230)),
        "deepskyblue"     => Some(c(  0, 191, 255)),
        "violet"          => Some(c(238, 130, 238)),
        "orchid"          => Some(c(218, 112, 214)),
        "plum"            => Some(c(221, 160, 221)),
        "thistle"         => Some(c(216, 191, 216)),
        "lavender"        => Some(c(230, 230, 250)),
        "indigo"          => Some(c( 75,   0, 130)),
        "darkviolet"      => Some(c(148,   0, 211)),
        "blueviolet"      => Some(c(138,  43, 226)),
        "mediumpurple"    => Some(c(147, 112, 219)),
        "mediumorchid"    => Some(c(186,  85, 211)),
        "darkorchid"      => Some(c(153,  50, 204)),
        "darkmagenta"     => Some(c(139,   0, 139)),
        "brown"           => Some(c(165,  42,  42)),
        "saddlebrown"     => Some(c(139,  69,  19)),
        "sienna"          => Some(c(160,  82,  45)),
        "chocolate"       => Some(c(210, 105,  30)),
        "peru"            => Some(c(205, 133,  63)),
        "burlywood"       => Some(c(222, 184, 135)),
        "sandybrown"      => Some(c(244, 164,  96)),
        "rosybrown"       => Some(c(188, 143, 143)),
        "snow"            => Some(c(255, 250, 250)),
        "honeydew"        => Some(c(240, 255, 240)),
        "mintcream"       => Some(c(245, 255, 250)),
        "azure"           => Some(c(240, 255, 255)),
        "aliceblue"       => Some(c(240, 248, 255)),
        "ghostwhite"      => Some(c(248, 248, 255)),
        "seashell"        => Some(c(255, 245, 238)),
        "beige"           => Some(c(245, 245, 220)),
        "oldlace"         => Some(c(253, 245, 230)),
        "floralwhite"     => Some(c(255, 250, 240)),
        "ivory"           => Some(c(255, 255, 240)),
        "antiquewhite"    => Some(c(250, 235, 215)),
        "linen"           => Some(c(250, 240, 230)),
        "lavenderblush"   => Some(c(255, 240, 245)),
        "mistyrose"       => Some(c(255, 228, 225)),
        "gainsboro"       => Some(c(220, 220, 220)),
        "lightgray"
        | "lightgrey"     => Some(c(211, 211, 211)),
        "darkgray"
        | "darkgrey"      => Some(c(169, 169, 169)),
        "dimgray"
        | "dimgrey"       => Some(c(105, 105, 105)),
        "slategray"
        | "slategrey"     => Some(c(112, 128, 144)),
        "lightslategray"
        | "lightslategrey" => Some(c(119, 136, 153)),
        "transparent"     => Some(Color::TRANSPARENT),
        "currentcolor"    => None, // must be resolved by caller
        _                 => None,
    }
}
