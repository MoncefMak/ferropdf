//! Inherited properties — which CSS properties propagate from parent to child.

/// Returns true if the given CSS property name is inherited.
pub fn is_inherited(prop: &str) -> bool {
    matches!(prop,
        // Typography (all inherited)
        "color" | "font" | "font-family" | "font-size" | "font-style"
        | "font-variant" | "font-weight" | "letter-spacing"
        | "line-height" | "text-align" | "text-decoration"
        | "text-indent" | "text-transform" | "white-space"
        | "word-spacing" | "direction" | "unicode-bidi"

        // Lists (inherited)
        | "list-style" | "list-style-type" | "list-style-position" | "list-style-image"

        // Table (inherited)
        | "border-collapse" | "border-spacing" | "caption-side"
        | "empty-cells" | "table-layout"

        // Visibility (inherited)
        | "visibility"

        // Cursor (irrelevant for PDF but technically inherited)
        | "cursor"

        // Page / print
        | "orphans" | "widows" | "page-break-inside"
    )
}
