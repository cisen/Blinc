//! SVG Animation Demo
//!
//! Demonstrates SVG animation capabilities:
//! - Phase 0: CSS transforms on SVG elements (rotate, scale)
//! - Phase 1: fill/stroke color animation via @keyframes
//! - Phase 3: stroke-dasharray/dashoffset line-drawing effect
//! - Phase 4: Path morphing (d-attribute animation)
//!
//! Run with: cargo run -p blinc_app --example svg_animation_demo --features windowed

use blinc_app::prelude::*;
use blinc_app::windowed::{WindowedApp, WindowedContext};
use blinc_core::Color;
use blinc_theme::{ColorToken, ThemeState};

/// Simple star SVG for transform/color demos
const STAR_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <path d="M50,5 L61,35 L95,35 L68,57 L79,90 L50,70 L21,90 L32,57 L5,35 L39,35 Z" fill="#fbbf24" stroke="#f59e0b" stroke-width="2"/>
</svg>"##;

/// Circle SVG for line-drawing demo
const CIRCLE_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <circle cx="50" cy="50" r="40" fill="none" stroke="#3b82f6" stroke-width="3"/>
</svg>"##;

/// Square path SVG for path morphing demo
const MORPH_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <path d="M20,20 L80,20 L80,80 L50,80 L20,80 Z" fill="#8b5cf6" stroke="#7c3aed" stroke-width="2"/>
</svg>"##;

/// Checkmark SVG for line-drawing
const CHECK_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <path d="M20,55 L40,75 L80,25" fill="none" stroke="#10b981" stroke-width="5" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##;

/// Mixed SVG with paths and circles for tag-name targeting demo
const MIXED_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <circle cx="50" cy="50" r="40" fill="#e2e8f0" stroke="#94a3b8" stroke-width="2"/>
  <path d="M30,50 L45,65 L70,35" fill="none" stroke="#475569" stroke-width="4" stroke-linecap="round" stroke-linejoin="round"/>
</svg>"##;

/// SVG with rect and circle for selective targeting
const SHAPES_SVG: &str = r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="0 0 100 100" width="100" height="100">
  <rect x="15" y="15" width="70" height="70" rx="8" fill="#dbeafe" stroke="#3b82f6" stroke-width="2"/>
  <circle cx="50" cy="50" r="20" fill="#bfdbfe" stroke="#2563eb" stroke-width="2"/>
</svg>"##;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let config = WindowConfig {
        title: "SVG Animation Demo".to_string(),
        width: 1100,
        height: 800,
        resizable: true,
        fullscreen: false,
        ..Default::default()
    };

    let mut css_loaded = false;

    WindowedApp::run(config, move |ctx| {
        if !css_loaded {
            ctx.add_css(SVG_CSS);
            css_loaded = true;
        }
        build_ui(ctx)
    })
}

const SVG_CSS: &str = r#"
/* Phase 0: CSS Transform Animation */
@keyframes spin {
    0%   { transform: rotate(0deg); }
    100% { transform: rotate(360deg); }
}
#spin-svg {
    animation: spin 3s linear infinite;
}

@keyframes pulse-scale {
    0%   { transform: scale(1.0); }
    50%  { transform: scale(1.3); }
    100% { transform: scale(1.0); }
}
#pulse-svg {
    animation: pulse-scale 2s ease-in-out infinite;
}

/* Phase 1: Fill/Stroke Color Animation */
@keyframes color-cycle {
    0%   { fill: #ef4444; stroke: #dc2626; }
    33%  { fill: #3b82f6; stroke: #2563eb; }
    66%  { fill: #10b981; stroke: #059669; }
    100% { fill: #ef4444; stroke: #dc2626; }
}
#color-svg {
    animation: color-cycle 4s ease-in-out infinite;
}

@keyframes glow-stroke {
    0%   { stroke: #fbbf24; stroke-width: 2; }
    50%  { stroke: #f43f5e; stroke-width: 5; }
    100% { stroke: #fbbf24; stroke-width: 2; }
}
#glow-svg {
    animation: glow-stroke 2s ease-in-out infinite;
}

/* Phase 3: Stroke Dash Animation (Line Drawing) */
@keyframes draw-circle {
    0%   { stroke-dashoffset: 251; }
    100% { stroke-dashoffset: 0; }
}
#draw-circle-svg {
    stroke-dasharray: 251;
    animation: draw-circle 3s ease-in-out infinite alternate;
}

@keyframes draw-check {
    0%   { stroke-dashoffset: 100; }
    100% { stroke-dashoffset: 0; }
}
#draw-check-svg {
    stroke-dasharray: 100;
    animation: draw-check 2s ease-out infinite alternate;
}

/* Phase 4: Path Morphing — both shapes must have the same number of segments */
@keyframes morph-shape {
    0%   { d: path("M20,20 L80,20 L80,80 L50,80 L20,80 Z"); }
    50%  { d: path("M50,10 L90,40 L75,85 L25,85 L10,40 Z"); }
    100% { d: path("M20,20 L80,20 L80,80 L50,80 L20,80 Z"); }
}
#morph-svg {
    animation: morph-shape 3s ease-in-out infinite;
}

/* Hover transitions */
#hover-fill-svg {
    fill: #6366f1;
    transition: fill 0.3s ease;
}
#hover-fill-svg:hover {
    fill: #f43f5e;
}

#hover-stroke-svg {
    stroke: #64748b;
    stroke-width: 2;
    transition: stroke 0.3s ease, stroke-width 0.3s ease;
}
#hover-stroke-svg:hover {
    stroke: #f59e0b;
    stroke-width: 5;
}

/* Tag-name CSS selectors — target specific SVG sub-element types */
#tag-check-svg path {
    stroke: #8b5cf6;
    stroke-width: 5;
}
#tag-check-svg circle {
    fill: #f3e8ff;
    stroke: #a78bfa;
}

#tag-shapes-svg rect {
    fill: #fef3c7;
    stroke: #f59e0b;
    stroke-width: 3;
}
#tag-shapes-svg circle {
    fill: #fed7aa;
    stroke: #ea580c;
    stroke-width: 2;
}
"#;

fn build_ui(ctx: &WindowedContext) -> impl ElementBuilder {
    let theme = ThemeState::get();
    let bg = theme.color(ColorToken::Background);

    div()
        .w(ctx.width)
        .h(ctx.height)
        .bg(bg)
        .flex_col()
        .child(header())
        .child(
            scroll().w_full().h(ctx.height - 60.0).child(
                div()
                    .w_full()
                    .p(24.0)
                    .flex_col()
                    .gap(32.0)
                    .child(transform_section())
                    .child(color_section())
                    .child(line_drawing_section())
                    .child(morph_section())
                    .child(hover_section())
                    .child(tag_selector_section()),
            ),
        )
}

fn header() -> Div {
    let theme = ThemeState::get();

    div()
        .w_full()
        .h(60.0)
        .bg(theme.color(ColorToken::Surface))
        .flex_row()
        .items_center()
        .px(24.0)
        .child(
            text("SVG Animation Demo")
                .size(24.0)
                .weight(FontWeight::Bold)
                .color(theme.color(ColorToken::TextPrimary)),
        )
}

fn section_card(title: &str, subtitle: &str) -> Div {
    let theme = ThemeState::get();

    div()
        .w_full()
        .bg(theme.color(ColorToken::Surface))
        .rounded(12.0)
        .p(20.0)
        .flex_col()
        .gap(16.0)
        .child(
            div()
                .flex_col()
                .gap(4.0)
                .child(
                    text(title)
                        .size(18.0)
                        .weight(FontWeight::SemiBold)
                        .color(theme.color(ColorToken::TextPrimary)),
                )
                .child(
                    text(subtitle)
                        .size(13.0)
                        .color(theme.color(ColorToken::TextSecondary)),
                ),
        )
}

fn demo_cell(label: &str, svg_content: &str, id: &str, size: f32) -> Div {
    let theme = ThemeState::get();

    div()
        .flex_col()
        .items_center()
        .gap(8.0)
        .child(
            div()
                .w(size + 20.0)
                .h(size + 20.0)
                .rounded(8.0)
                .bg(Color::rgba(0.0, 0.0, 0.0, 0.2))
                .flex()
                .items_center()
                .justify_center()
                .child(svg(svg_content).square(size).id(id)),
        )
        .child(
            text(label)
                .size(12.0)
                .color(theme.color(ColorToken::TextSecondary)),
        )
}

/// Phase 0: CSS transforms on SVGs
fn transform_section() -> Div {
    section_card(
        "Phase 0: CSS Transforms",
        "rotate() and scale() animations applied directly to SVG elements",
    )
    .child(
        div()
            .flex_row()
            .gap(48.0)
            .justify_center()
            .child(demo_cell("Spin (rotate)", STAR_SVG, "spin-svg", 80.0))
            .child(demo_cell("Pulse (scale)", STAR_SVG, "pulse-svg", 80.0)),
    )
}

/// Phase 1: Fill/stroke color animation
fn color_section() -> Div {
    section_card(
        "Phase 1: Fill & Stroke Animation",
        "fill and stroke colors animated via @keyframes",
    )
    .child(
        div()
            .flex_row()
            .gap(48.0)
            .justify_center()
            .child(demo_cell("Color Cycle", STAR_SVG, "color-svg", 80.0))
            .child(demo_cell("Glow Stroke", STAR_SVG, "glow-svg", 80.0)),
    )
}

/// Phase 3: Stroke dash animation (line drawing)
fn line_drawing_section() -> Div {
    section_card(
        "Phase 3: Line Drawing Effect",
        "stroke-dasharray + stroke-dashoffset animation for SVG line drawing",
    )
    .child(
        div()
            .flex_row()
            .gap(48.0)
            .justify_center()
            .child(demo_cell(
                "Draw Circle",
                CIRCLE_SVG,
                "draw-circle-svg",
                80.0,
            ))
            .child(demo_cell("Draw Check", CHECK_SVG, "draw-check-svg", 80.0)),
    )
}

/// Phase 4: Path morphing
fn morph_section() -> Div {
    section_card(
        "Phase 4: Path Morphing",
        "d: path() animation morphs between shapes via cubic bezier interpolation",
    )
    .child(div().flex_row().gap(48.0).justify_center().child(demo_cell(
        "Square \u{2194} Pentagon",
        MORPH_SVG,
        "morph-svg",
        80.0,
    )))
}

/// Hover transitions
fn hover_section() -> Div {
    section_card(
        "Hover Transitions",
        "CSS transitions on fill and stroke (hover to see effect)",
    )
    .child(
        div()
            .flex_row()
            .gap(48.0)
            .justify_center()
            .child(demo_cell(
                "Fill Transition",
                STAR_SVG,
                "hover-fill-svg",
                80.0,
            ))
            .child(demo_cell(
                "Stroke Transition",
                STAR_SVG,
                "hover-stroke-svg",
                80.0,
            )),
    )
}

/// Tag-name CSS selectors targeting SVG sub-elements
fn tag_selector_section() -> Div {
    section_card(
        "Tag-Name CSS Selectors",
        "CSS selectors like `path { }` and `circle { }` target specific SVG sub-element types",
    )
    .child(
        div()
            .flex_row()
            .gap(48.0)
            .justify_center()
            .child(demo_cell(
                "path=purple, circle=lilac",
                MIXED_SVG,
                "tag-check-svg",
                80.0,
            ))
            .child(demo_cell(
                "rect=amber, circle=orange",
                SHAPES_SVG,
                "tag-shapes-svg",
                80.0,
            )),
    )
}
