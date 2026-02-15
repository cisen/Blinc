//! Semantic @flow Demo
//!
//! Demonstrates the semantic step/chain/use system for @flow shaders.
//! Uses `step`, `chain`, and raw `node` syntax together to create a
//! layered noise visualization with pointer-reactive color ramping.
//!
//! Run with: cargo run -p blinc_app --example semantic_flow_demo --features windowed

use blinc_app::prelude::*;
use blinc_app::windowed::{WindowedApp, WindowedContext};
use blinc_core::{Color, Shadow};

const STYLESHEET: &str = r#"
    /* ====== Semantic flow: steps + chains + raw nodes ====== */

    /* Step demo: noise pattern with color ramp */
    @flow terrain {
        target: fragment;
        input uv: builtin(uv);
        input time: builtin(time);

        /* Semantic step — procedural noise */
        step noise: pattern-noise {
            scale: 4.0;
            detail: 6;
            animation: time * 0.3;
        }

        /* Raw node for contrast adjustment */
        node contrast = smoothstep(0.2, 0.8, noise);

        /* Semantic step — color mapping from scalar to palette */
        step palette: color-ramp {
            source: contrast;
            stops: #1a3a5a 0.0, #4488aa 0.35, #88cc66 0.55, #ccdd44 0.75, #ffffff 1.0;
        }

        output color = palette;
    }

    /* Chain demo: ripple effect with falloff */
    @flow ripple {
        target: fragment;
        input uv: builtin(uv);
        input time: builtin(time);

        chain effect:
            pattern-ripple(center: vec2(0.5, 0.5), density: 25.0, speed: 3.0)
            | adjust-falloff(radius: 0.5)
            ;

        /* Map scalar to blue-white gradient */
        step tint: color-ramp {
            source: effect;
            stops: #002244 0.0, #0066cc 0.4, #88ddff 0.7, #ffffff 1.0;
        }

        output color = tint;
    }

    /* Mixed demo: raw math + semantic step */
    @flow waves {
        target: fragment;
        input uv: builtin(uv);
        input time: builtin(time);

        /* Raw math for custom wave pattern */
        node cx = uv.x - 0.5;
        node cy = uv.y - 0.5;
        node dist = length(vec2(cx, cy));
        node wave1 = sin(dist * 30.0 - time * 2.0) * 0.5 + 0.5;
        node wave2 = sin(uv.x * 15.0 + time * 1.5) * 0.5 + 0.5;
        node combined = wave1 * 0.6 + wave2 * 0.4;

        /* Semantic color ramp on the combined wave */
        step colored: color-ramp {
            source: combined;
            stops: #030221 0.0, #140d4c 0.3, #2c5ac7 0.6, #407dee 0.8, #d9eef2 1.0;
        }

        output color = colored;
    }

    /* ====== Card styling ====== */
    #terrain-card {
        flow: terrain;
        border-radius: 24px;
    }
    #ripple-card {
        flow: ripple;
        border-radius: 24px;
    }
    #waves-card {
        flow: waves;
        border-radius: 24px;
    }
"#;

fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::from_default_env()
                .add_directive(tracing::Level::INFO.into()),
        )
        .init();

    let config = WindowConfig {
        title: "Semantic Flow Demo".to_string(),
        width: 1100,
        height: 600,
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

fn build_ui(ctx: &WindowedContext) -> impl ElementBuilder {
    let bg = Color::rgba(0.12, 0.12, 0.14, 1.0);
    let card_w = 280.0;
    let card_h = 280.0;
    let label_color = Color::rgba(0.85, 0.85, 0.90, 1.0);
    let sub_color = Color::rgba(0.55, 0.55, 0.60, 1.0);

    div()
        .w(ctx.width)
        .h(ctx.height)
        .bg(bg)
        .p(16.0)
        .items_center()
        .justify_center()
        .child(
            div()
                .flex_row()
                .gap(16.0)
                .items_center()
                .child(card(
                    "terrain-card",
                    "Terrain",
                    "step + node + step",
                    card_w,
                    card_h,
                    label_color,
                    sub_color,
                ))
                .child(card(
                    "ripple-card",
                    "Ripple",
                    "chain + step",
                    card_w,
                    card_h,
                    label_color,
                    sub_color,
                ))
                .child(card(
                    "waves-card",
                    "Waves",
                    "node + step",
                    card_w,
                    card_h,
                    label_color,
                    sub_color,
                )),
        )
}

fn card(
    id: &str,
    title: &str,
    subtitle: &str,
    w: f32,
    h: f32,
    label_color: Color,
    sub_color: Color,
) -> impl ElementBuilder {
    div()
        .flex_col()
        .items_center()
        .gap(12.0)
        .child(
            div()
                .id(id)
                .w(w)
                .h(h)
                .bg(Color::rgba(0.2, 0.2, 0.25, 1.0))
                .shadow(Shadow::new(0.0, 4.0, 20.0, Color::rgba(0.0, 0.0, 0.0, 0.4))),
        )
        .child(
            text(title)
                .size(20.0)
                .weight(FontWeight::Bold)
                .color(label_color),
        )
        .child(text(subtitle).size(14.0).color(sub_color))
}
