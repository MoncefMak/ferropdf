//! Standard PDF font metrics from Adobe's AFM files.
//!
//! Character widths are in 1/1000 of text space units (at 1pt font size,
//! 1 unit = 1/1000 pt). Multiply by font_size / 1000.0 to get width in points.

/// Width table: maps ASCII code points (32..=126) to width in 1/1000 em units.
type WidthTable = [u16; 95]; // indices 0..94 for chars 32..126

// ── Helvetica ──────────────────────────────────────────────────────────────

static HELVETICA: WidthTable = [
    278,  // 32 space
    278,  // 33 !
    355,  // 34 "
    556,  // 35 #
    556,  // 36 $
    889,  // 37 %
    667,  // 38 &
    191,  // 39 '
    333,  // 40 (
    333,  // 41 )
    389,  // 42 *
    584,  // 43 +
    278,  // 44 ,
    333,  // 45 -
    278,  // 46 .
    278,  // 47 /
    556,  // 48 0
    556,  // 49 1
    556,  // 50 2
    556,  // 51 3
    556,  // 52 4
    556,  // 53 5
    556,  // 54 6
    556,  // 55 7
    556,  // 56 8
    556,  // 57 9
    278,  // 58 :
    278,  // 59 ;
    584,  // 60 <
    584,  // 61 =
    584,  // 62 >
    556,  // 63 ?
    1015, // 64 @
    667,  // 65 A
    667,  // 66 B
    722,  // 67 C
    722,  // 68 D
    667,  // 69 E
    611,  // 70 F
    778,  // 71 G
    722,  // 72 H
    278,  // 73 I
    500,  // 74 J
    667,  // 75 K
    556,  // 76 L
    833,  // 77 M
    722,  // 78 N
    778,  // 79 O
    667,  // 80 P
    778,  // 81 Q
    722,  // 82 R
    667,  // 83 S
    611,  // 84 T
    722,  // 85 U
    667,  // 86 V
    944,  // 87 W
    667,  // 88 X
    667,  // 89 Y
    611,  // 90 Z
    278,  // 91 [
    278,  // 92 \
    278,  // 93 ]
    469,  // 94 ^
    556,  // 95 _
    333,  // 96 `
    556,  // 97 a
    556,  // 98 b
    500,  // 99 c
    556,  // 100 d
    556,  // 101 e
    278,  // 102 f
    556,  // 103 g
    556,  // 104 h
    222,  // 105 i
    222,  // 106 j
    500,  // 107 k
    222,  // 108 l
    833,  // 109 m
    556,  // 110 n
    556,  // 111 o
    556,  // 112 p
    556,  // 113 q
    333,  // 114 r
    500,  // 115 s
    278,  // 116 t
    556,  // 117 u
    500,  // 118 v
    722,  // 119 w
    500,  // 120 x
    500,  // 121 y
    500,  // 122 z
    334,  // 123 {
    260,  // 124 |
    334,  // 125 }
    584,  // 126 ~
];

static HELVETICA_BOLD: WidthTable = [
    278, 333, 474, 556, 556, 889, 722, 238, 333, 333, 389, 584, 278, 333, 278, 278, 556, 556, 556,
    556, 556, 556, 556, 556, 556, 556, 333, 333, 584, 584, 584, 611, 975, 722, 722, 722, 722, 667,
    611, 778, 722, 278, 556, 722, 611, 833, 722, 778, 667, 778, 722, 667, 611, 722, 667, 944, 667,
    667, 611, 333, 278, 333, 584, 556, 333, 556, 611, 556, 611, 556, 333, 611, 611, 278, 278, 556,
    278, 889, 611, 611, 611, 611, 389, 556, 333, 611, 556, 778, 556, 556, 500, 389, 280, 389, 584,
];

// ── Times Roman ────────────────────────────────────────────────────────────

static TIMES_ROMAN: WidthTable = [
    250, // space
    333, 408, 500, 500, 833, 778, 180, 333, 333, 500, 564, 250, 333, 250, 278, 500, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 278, 278, 564, 564, 564, 444, 921, 722, 667, 667, 722, 611, 556,
    722, 722, 333, 389, 722, 611, 889, 722, 722, 556, 722, 667, 556, 611, 722, 722, 944, 722, 722,
    611, 333, 278, 333, 469, 500, 333, 444, 500, 444, 500, 444, 333, 500, 500, 278, 278, 500, 278,
    778, 500, 500, 500, 500, 333, 389, 278, 500, 500, 722, 500, 500, 444, 480, 200, 480, 541,
];

static TIMES_BOLD: WidthTable = [
    250, 333, 555, 500, 500, 1000, 833, 278, 333, 333, 500, 570, 250, 333, 250, 278, 500, 500, 500,
    500, 500, 500, 500, 500, 500, 500, 333, 333, 570, 570, 570, 500, 930, 722, 667, 722, 722, 667,
    611, 778, 778, 389, 500, 778, 667, 944, 722, 778, 611, 778, 722, 556, 667, 722, 722, 1000, 722,
    722, 667, 333, 278, 333, 581, 500, 333, 500, 556, 444, 556, 444, 333, 500, 556, 278, 333, 556,
    278, 833, 556, 500, 556, 556, 444, 389, 333, 556, 500, 722, 500, 500, 444, 394, 220, 394, 520,
];

// ── Courier (monospaced — all 600) ────────────────────────────────────────

static COURIER: WidthTable = [600; 95];

// ── Font metrics lookup ────────────────────────────────────────────────────

/// Get the width of a character in 1/1000 em units for a given PDF built-in font.
/// Returns a default width (500 for proportional, 600 for monospace) for unknown chars.
pub fn char_width(font_name: &str, ch: char) -> u16 {
    let table = get_width_table(font_name);
    let code = ch as u32;
    if (32..=126).contains(&code) {
        table[(code - 32) as usize]
    } else {
        // Default width for non-ASCII characters
        if font_name.starts_with("Courier") {
            600
        } else {
            500
        }
    }
}

/// Get the width table for a built-in font name.
fn get_width_table(font_name: &str) -> &'static WidthTable {
    match font_name {
        "Helvetica" | "Helvetica-Oblique" => &HELVETICA,
        "Helvetica-Bold" | "Helvetica-BoldOblique" => &HELVETICA_BOLD,
        "Times-Roman" | "Times-Italic" => &TIMES_ROMAN,
        "Times-Bold" | "Times-BoldItalic" => &TIMES_BOLD,
        _ if font_name.starts_with("Courier") => &COURIER,
        _ => &HELVETICA, // fallback
    }
}

/// Measure the width of a string in points for a given built-in PDF font and size.
pub fn measure_text_width(text: &str, font_name: &str, font_size_pt: f64) -> f64 {
    let total_units: u32 = text.chars().map(|c| char_width(font_name, c) as u32).sum();
    (total_units as f64) * font_size_pt / 1000.0
}

/// Measure the width of a string in pixels (at 96 dpi).
pub fn measure_text_width_px(text: &str, font_name: &str, font_size_px: f64) -> f64 {
    let font_size_pt = font_size_px * 72.0 / 96.0;
    let width_pt = measure_text_width(text, font_name, font_size_pt);
    // Convert pt back to px
    width_pt * 96.0 / 72.0
}

/// Average character width in pixels for a given font (used for quick estimates).
pub fn average_char_width_px(font_name: &str, font_size_px: f64) -> f64 {
    // Use width of 'x' as a reasonable average
    let x_width = char_width(font_name, 'x') as f64;
    x_width * font_size_px / 1000.0 * 72.0 / 96.0 * 96.0 / 72.0
}

/// Word-wrap text based on actual font metrics. Returns wrapped lines.
pub fn wrap_text_measured(
    text: &str,
    font_name: &str,
    font_size_px: f64,
    max_width_px: f64,
) -> Vec<String> {
    if text.is_empty() || max_width_px <= 0.0 {
        return vec![text.to_string()];
    }

    // Add a small tolerance (0.5px) to avoid floating-point accumulation
    // mismatches between measuring the full string vs. measuring per-word.
    let effective_max = max_width_px + 0.5;

    let mut lines = Vec::new();
    let mut current_line = String::new();
    let mut current_width = 0.0;
    let space_width = measure_text_width_px(" ", font_name, font_size_px);

    for word in text.split_whitespace() {
        let word_width = measure_text_width_px(word, font_name, font_size_px);

        if current_line.is_empty() {
            // First word on line — always add it even if it overflows
            current_line = word.to_string();
            current_width = word_width;
        } else if current_width + space_width + word_width <= effective_max {
            current_line.push(' ');
            current_line.push_str(word);
            current_width += space_width + word_width;
        } else {
            lines.push(current_line);
            current_line = word.to_string();
            current_width = word_width;
        }
    }

    if !current_line.is_empty() {
        lines.push(current_line);
    }

    if lines.is_empty() {
        lines.push(String::new());
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_helvetica_space_width() {
        assert_eq!(char_width("Helvetica", ' '), 278);
    }

    #[test]
    fn test_courier_monospaced() {
        assert_eq!(char_width("Courier", 'a'), 600);
        assert_eq!(char_width("Courier", 'W'), 600);
        assert_eq!(char_width("Courier", ' '), 600);
    }

    #[test]
    fn test_measure_text() {
        // "Hello" in Helvetica at 12pt
        let width = measure_text_width("Hello", "Helvetica", 12.0);
        // H=722, e=556, l=222, l=222, o=556 = 2278 units
        // 2278 * 12 / 1000 = 27.336 pt
        assert!((width - 27.336).abs() < 0.01);
    }

    #[test]
    fn test_wrap_text() {
        let lines = wrap_text_measured("Hello World", "Helvetica", 16.0, 60.0);
        assert!(lines.len() >= 2);
    }

    #[test]
    fn test_non_ascii_fallback() {
        let w = char_width("Helvetica", 'é');
        assert_eq!(w, 500); // default fallback
    }

    #[test]
    fn test_wrap_exact_width() {
        // wrap_text_measured must not break text when available_width equals
        // the measured string width (floating-point tolerance).
        let font_name = "Helvetica-Bold";
        let font_size = 14.0 * 96.0 / 72.0;
        let text = "HTML and CSS";
        let width = measure_text_width_px(text, font_name, font_size);
        let lines = wrap_text_measured(text, font_name, font_size, width);
        assert_eq!(lines, vec!["HTML and CSS"]);
    }
}
