//! # Blinc Tabler Icons
//!
//! [Tabler](https://tabler.io/icons)-based icon library for Blinc UI framework.
//!
//! ~5000 outline and ~1000 filled icons available as `pub const` values.
//! Unused icons are automatically eliminated by Rust's Dead Code Elimination (DCE).
//!
//! ## Usage
//!
//! ```ignore
//! use blinc_tabler_icons::{outline, filled, to_svg, to_svg_filled};
//!
//! // Outline icon (stroke-based)
//! let svg = to_svg(outline::HOME, 24.0);
//!
//! // Filled icon (fill-based)
//! let svg = to_svg_filled(filled::HOME, 24.0);
//! ```

// Generated icon constants modules
// NOTE: these files are generated and intentionally not rustfmt'd.
#[rustfmt::skip]
pub mod outline;
#[rustfmt::skip]
pub mod filled;

/// Default Tabler viewBox (all icons are 24x24)
pub const VIEW_BOX: (f32, f32, f32, f32) = (0.0, 0.0, 24.0, 24.0);

/// Default stroke width for Tabler outline icons
pub const STROKE_WIDTH: f32 = 2.0;

/// Generate a complete SVG string from outline icon path data
///
/// Wraps the inner elements with stroke-based SVG attributes
/// (`stroke="currentColor"`, `fill="none"`).
///
/// # Arguments
/// * `path_data` - The SVG inner elements (from `outline::*` constants)
/// * `size` - The width and height of the SVG in pixels
///
/// # Example
/// ```ignore
/// let svg = blinc_tabler_icons::to_svg(outline::HOME, 24.0);
/// ```
pub fn to_svg(path_data: &str, size: f32) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{path_data}</svg>"#
    )
}

/// Generate a complete SVG string from filled icon path data
///
/// Wraps the inner elements with fill-based SVG attributes
/// (`fill="currentColor"`, `stroke="none"`).
///
/// # Arguments
/// * `path_data` - The SVG inner elements (from `filled::*` constants)
/// * `size` - The width and height of the SVG in pixels
///
/// # Example
/// ```ignore
/// let svg = blinc_tabler_icons::to_svg_filled(filled::HOME, 24.0);
/// ```
pub fn to_svg_filled(path_data: &str, size: f32) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" fill="currentColor" stroke="none">{path_data}</svg>"#
    )
}

/// Generate outline SVG with custom stroke width
pub fn to_svg_with_stroke(path_data: &str, size: f32, stroke_width: f32) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="{stroke_width}" stroke-linecap="round" stroke-linejoin="round">{path_data}</svg>"#
    )
}

/// Generate outline SVG with custom color (for non-currentColor usage)
pub fn to_svg_colored(path_data: &str, size: f32, color: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" fill="none" stroke="{color}" stroke-width="2" stroke-linecap="round" stroke-linejoin="round">{path_data}</svg>"#
    )
}

/// Generate filled SVG with custom color
pub fn to_svg_filled_colored(path_data: &str, size: f32, color: &str) -> String {
    format!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{size}" height="{size}" viewBox="0 0 24 24" fill="{color}" stroke="none">{path_data}</svg>"#
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_to_svg_outline() {
        let svg = to_svg(outline::HOME, 24.0);
        assert!(svg.contains("viewBox=\"0 0 24 24\""));
        assert!(svg.contains("width=\"24\""));
        assert!(svg.contains("stroke-width=\"2\""));
        assert!(svg.contains("fill=\"none\""));
        assert!(svg.contains("stroke=\"currentColor\""));
    }

    #[test]
    fn test_to_svg_filled() {
        let svg = to_svg_filled(filled::HOME, 24.0);
        assert!(svg.contains("viewBox=\"0 0 24 24\""));
        assert!(svg.contains("fill=\"currentColor\""));
        assert!(svg.contains("stroke=\"none\""));
    }

    #[test]
    fn test_to_svg_with_stroke() {
        let svg = to_svg_with_stroke(outline::HOME, 16.0, 1.5);
        assert!(svg.contains("width=\"16\""));
        assert!(svg.contains("stroke-width=\"1.5\""));
    }

    #[test]
    fn test_to_svg_colored() {
        let svg = to_svg_colored(outline::HOME, 24.0, "#ff0000");
        assert!(svg.contains("stroke=\"#ff0000\""));
    }

    #[test]
    fn test_to_svg_filled_colored() {
        let svg = to_svg_filled_colored(filled::HOME, 24.0, "#00ff00");
        assert!(svg.contains("fill=\"#00ff00\""));
    }
}
