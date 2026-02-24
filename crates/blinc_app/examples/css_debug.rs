//! CSS Debug Example
//!
//! Tests three known CSS issues:
//! 1. var() not picking up values from :root
//! 2. width/height percentage not working
//! 3. Text not inheriting color from parent div
//!
//! Run with: cargo run -p blinc_app --example css_debug --features windowed

use blinc_app::prelude::*;
use blinc_app::windowed::{WindowedApp, WindowedContext};

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_max_level(tracing::Level::DEBUG)
        .init();

    let config = WindowConfig {
        title: "CSS Debug — var(), %, color inheritance".to_string(),
        width: 900,
        height: 700,
        resizable: true,
        ..Default::default()
    };

    let mut css_loaded = false;
    WindowedApp::run(config, move |ctx| {
        if !css_loaded {
            ctx.add_css(STYLESHEET);
            css_loaded = true;
        }
        build_ui(ctx)
    })
}

const STYLESHEET: &str = r#"
        :root {
            --brand-color: #3b82f6;
            --accent-color: #f59e0b;
            --text-color: #ffffff;
            --card-radius: 16px;
            --card-padding: 24px;
        }

        /* === TEST 1: var() from :root === */
        #var-test {
            background: var(--brand-color);
            border-radius: var(--card-radius);
            padding: var(--card-padding);
        }

        #var-test-accent {
            background: var(--accent-color);
            border-radius: 12px;
            padding: 16px;
        }

        #var-test-fallback {
            background: var(--nonexistent, #ef4444);
            border-radius: 12px;
            padding: 16px;
        }

        /* === TEST 2: Percentage width/height === */
        #percent-container {
            background: #1e293b;
            border-radius: 12px;
            padding: 16px;
        }

        #percent-half {
            width: 50%;
            background: #3b82f6;
            border-radius: 8px;
            padding: 12px;
        }

        #percent-full {
            width: 100%;
            background: #8b5cf6;
            border-radius: 8px;
            padding: 12px;
        }

        #percent-third {
            width: 33.3%;
            background: #10b981;
            border-radius: 8px;
            padding: 12px;
        }

        /* === TEST 3: Text color inheritance === */
        #inherit-parent {
            color: #3b82f6;
            background: #1e293b;
            border-radius: 12px;
            padding: 16px;
        }

        #inherit-nested {
            color: #f59e0b;
        }

        .white-text {
            color: #ffffff;
        }

        /* Section styles */
        #root {
            background: #0f172a;
        }

        .section {
            background: #1e293b;
            border-radius: 16px;
            padding: 24px;
        }
"#;

fn build_ui(ctx: &WindowedContext) -> impl ElementBuilder {
    div()
        .w(ctx.width)
        .h(ctx.height)
        .id("root")
        .flex_col()
        .gap(5.0)
        .p(5.0)
        .overflow_y_scroll()
        .child(
            text("CSS Debug Tests")
                .size(28.0)
                .weight(FontWeight::Bold)
                .color(Color::WHITE),
        )
        .child(test_var_from_root())
        .child(test_percentage_dimensions())
        .child(test_color_inheritance())
}

/// TEST 1: var() should resolve values defined in :root
fn test_var_from_root() -> impl ElementBuilder {
    div()
        .class("section")
        .flex_col()
        .gap(3.0)
        .child(
            div().child(
                text("Test 1: var() from :root")
                    .size(20.0)
                    .weight(FontWeight::Bold)
                    .color(Color::rgba(0.58, 0.64, 0.72, 1.0)),
            ),
        )
        .child(
            div()
                .flex_row()
                .gap(3.0)
                // Should be blue (#3b82f6) from --brand-color
                .child(
                    div().id("var-test").child(
                        text("var(--brand-color)\nExpect: Blue bg")
                            .size(14.0)
                            .color(Color::WHITE),
                    ),
                )
                // Should be amber (#f59e0b) from --accent-color
                .child(
                    div().id("var-test-accent").child(
                        text("var(--accent-color)\nExpect: Amber bg")
                            .size(14.0)
                            .color(Color::WHITE),
                    ),
                )
                // Should be red (#ef4444) from fallback
                .child(
                    div().id("var-test-fallback").child(
                        text("var(--nonexistent, #ef4444)\nExpect: Red bg (fallback)")
                            .size(14.0)
                            .color(Color::WHITE),
                    ),
                ),
        )
}

/// TEST 2: Percentage width/height should work in CSS
fn test_percentage_dimensions() -> impl ElementBuilder {
    div()
        .class("section")
        .flex_col()
        .gap(3.0)
        .child(
            div().child(
                text("Test 2: Percentage Width")
                    .size(20.0)
                    .weight(FontWeight::Bold)
                    .color(Color::rgba(0.58, 0.64, 0.72, 1.0)),
            ),
        )
        .child(
            div()
                .id("percent-container")
                .w_full()
                .flex_col()
                .gap(2.0)
                // 50% width
                .child(
                    div().id("percent-half").child(
                        text("width: 50% — should be half")
                            .size(14.0)
                            .color(Color::WHITE),
                    ),
                )
                // 100% width
                .child(
                    div().id("percent-full").child(
                        text("width: 100% — should be full")
                            .size(14.0)
                            .color(Color::WHITE),
                    ),
                )
                // 33.3% width
                .child(
                    div().id("percent-third").child(
                        text("width: 33.3% — should be one-third")
                            .size(14.0)
                            .color(Color::WHITE),
                    ),
                ),
        )
}

/// TEST 3: Text should inherit color from parent div
fn test_color_inheritance() -> impl ElementBuilder {
    div()
        .class("section")
        .flex_col()
        .gap(3.0)
        .child(
            div().child(
                text("Test 3: Text Color Inheritance")
                    .size(20.0)
                    .weight(FontWeight::Bold)
                    .color(Color::rgba(0.58, 0.64, 0.72, 1.0)),
            ),
        )
        .child(
            div()
                .flex_col()
                .gap(2.0)
                // Parent has color: blue — child text should inherit it
                .child(
                    div()
                        .id("inherit-parent")
                        .flex_col()
                        .gap(2.0)
                        .child(
                            text("This text should be BLUE (inherited from parent #inherit-parent { color: #3b82f6 })")
                                .size(14.0),
                        )
                        // Nested div with its own color
                        .child(
                            div()
                                .id("inherit-nested")
                                .child(
                                    text("This text should be AMBER (inherited from #inherit-nested { color: #f59e0b })")
                                        .size(14.0),
                                ),
                        ),
                )
                // Class-based color
                .child(
                    div()
                        .class("white-text")
                        .child(
                            text("This text should be WHITE (inherited from .white-text { color: #fff })")
                                .size(14.0),
                        ),
                )
                // Control: direct color on text (should always work)
                .child(
                    text("Control: direct .color(Color::GREEN) — should be green")
                        .size(14.0)
                        .color(Color::GREEN),
                ),
        )
}
