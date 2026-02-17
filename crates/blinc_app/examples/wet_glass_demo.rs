//! Wet Glass Demo
//!
//! Procedural wet-window effect with real light refraction through water drops.
//! Uses `sample_scene()` to read the background and distort it through
//! procedural Worley-noise drops, streaks, and condensation fog.
//!
//! Run with: cargo run -p blinc_app --example wet_glass_demo --features windowed

use blinc_app::prelude::*;
use blinc_app::windowed::{WindowedApp, WindowedContext};
use blinc_core::Color;

const STYLESHEET: &str = r#"
    /* ====== Wet glass with real refraction ======

       Physical model:
       - Condensation fog: thin semi-transparent haze over the whole surface
       - Water drops: convex lenses that REFRACT the background (via sample_scene)
       - Running streaks: vertical trails cleared by gravity
       - Specular highlights: tiny bright points on curved water surfaces

       Key: drops are opaque in the shader (alpha=1) so they REPLACE the
       un-refracted background with a UV-shifted version from sample_scene().
       Fog areas use low alpha → background shows through with haze.
    */

    @flow wetglass {
        target: fragment;
        input uv: builtin(uv);
        input time: builtin(time);
        input resolution: builtin(resolution);

        node aspect = resolution.x / max(resolution.y, 1.0);
        node uv_a = vec2(uv.x * aspect, uv.y);

        /* ── Moisture field (controls where drops appear) ── */
        step mist: pattern-noise {
            scale: 3.0;
            detail: 5;
            animation: time * 0.008;
        }
        node grav = smoothstep(0.0, 1.0, uv.y);
        node moist = mist * (0.3 + grav * 0.7);

        /* ── Drop distance fields (denser, smaller drops) ── */
        node w1 = worley(uv_a, 18.0);
        node drop1 = smoothstep(0.055, 0.004, w1) * step(0.32, moist);

        node uv2 = uv_a + vec2(0.38, 0.21);
        node w2 = worley(uv2, 35.0);
        node drop2 = smoothstep(0.035, 0.003, w2) * step(0.22, moist);

        node uv3 = uv_a + vec2(0.17, 0.63);
        node w3 = worley(uv3, 60.0);
        node drop3 = smoothstep(0.022, 0.002, w3) * step(0.15, moist);

        /* Running streaks — stretched Worley */
        node uv_s = vec2(uv.x * aspect * 12.0, uv.y * 2.0);
        node ws = worley(uv_s, 1.0);
        node streak = smoothstep(0.05, 0.002, ws) * step(0.30, moist) * grav;

        node drops = clamp(drop1 + drop2 * 0.8 + drop3 * 0.5
                           + streak * 0.7, 0.0, 1.0);

        /* ── Refraction: gradient × distance = convex lens ── */
        /* For a dome-shaped drop, the UV offset should grow
           from center (0) to edge (max).  gradient ≈ unit dir,
           distance = 0 at center → offset ∝ distance. */
        node eps = 0.002;

        /* Large drops gradient */
        node w1_px = worley(uv_a + vec2(eps, 0.0), 18.0);
        node w1_nx = worley(uv_a - vec2(eps, 0.0), 18.0);
        node w1_py = worley(uv_a + vec2(0.0, eps), 18.0);
        node w1_ny = worley(uv_a - vec2(0.0, eps), 18.0);
        node nx1 = w1_px - w1_nx;
        node ny1 = w1_py - w1_ny;

        /* Medium drops gradient */
        node w2_px = worley(uv2 + vec2(eps, 0.0), 35.0);
        node w2_nx = worley(uv2 - vec2(eps, 0.0), 35.0);
        node w2_py = worley(uv2 + vec2(0.0, eps), 35.0);
        node w2_ny = worley(uv2 - vec2(0.0, eps), 35.0);
        node nx2 = w2_px - w2_nx;
        node ny2 = w2_py - w2_ny;

        /* Streak gradient */
        node ws_px = worley(uv_s + vec2(eps, 0.0), 1.0);
        node ws_nx = worley(uv_s - vec2(eps, 0.0), 1.0);
        node ws_py = worley(uv_s + vec2(0.0, eps), 1.0);
        node ws_ny = worley(uv_s - vec2(0.0, eps), 1.0);
        node nxs = ws_px - ws_nx;
        node nys = ws_py - ws_ny;

        /* Lens offset = gradient_dir × distance × strength × mask */
        node lens = 0.15;
        node ox = nx1 * w1 * lens * drop1
                + nx2 * w2 * lens * 0.5 * drop2
                + nxs * ws * lens * 0.3 * streak;
        node oy = ny1 * w1 * lens * drop1
                + ny2 * w2 * lens * 0.5 * drop2
                + nys * ws * lens * 0.3 * streak;

        /* Frost distortion in fog areas (noise-based UV jitter) */
        step frost_x: pattern-noise {
            scale: 30.0;
            detail: 2;
            animation: 0.0;
        }
        step frost_y: pattern-noise {
            scale: 30.0;
            detail: 2;
            animation: 100.0;
        }
        node frost = 0.003 * (1.0 - drops);
        node fx = (frost_x - 0.5) * frost;
        node fy = (frost_y - 0.5) * frost;

        /* ── Sample scene with combined offset ──────────── */
        node scene = sample_scene(uv + vec2(ox + fx, oy + fy));

        /* ── Specular highlight (one scale, drop-only) ──── */
        node gs1 = vec2(uv.x * aspect * 20.0, uv.y * 20.0);
        node cs1 = floor(gs1);
        node fs1 = fract(gs1) - vec2(0.5, 0.5);
        node h1a = fract(sin(dot(cs1, vec2(127.1, 311.7))) * 43758.5);
        node h1b = fract(sin(dot(cs1, vec2(269.5, 183.3))) * 43758.5);
        node h1c = fract(sin(dot(cs1, vec2(97.3, 157.1))) * 43758.5);
        node d1 = fs1 - vec2(h1a - 0.5, h1b - 0.5) * 0.4;
        node spec = smoothstep(0.025, 0.0, length(d1))
                    * step(0.5, h1c) * drops;

        /* ── Composite ──────────────────────────────────── */
        /* Everywhere: sample_scene (refracted in drops, frosted in fog).
           Fog areas: very thin white tint at low alpha.
           Drop areas: opaque (alpha=1) refracted scene + spec. */
        node fog_a = 0.06 + mist * 0.04;
        node tint = (1.0 - drops) * 0.1;

        node out_r = scene.x * (1.0 - tint) + tint + spec * 0.5;
        node out_g = scene.y * (1.0 - tint) + tint + spec * 0.5;
        node out_b = scene.z * (1.0 - tint) + tint + spec * 0.5;
        node out_a = mix(fog_a, 1.0, drops);

        output color = vec4(out_r, out_g, out_b, out_a);
    }

    #glass-layer {
        flow: wetglass;
        border-radius: 16px;
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
        title: "Wet Glass Demo".to_string(),
        width: 900,
        height: 650,
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
    let w = ctx.width;
    let h = ctx.height;

    div()
        .w(w)
        .h(h)
        .bg(Color::rgba(0.08, 0.08, 0.10, 1.0))
        .items_center()
        .justify_center()
        .child(
            // Container with relative positioning for overlay stacking
            div()
                .w(800.0)
                .h(550.0)
                .relative()
                .rounded(16.0)
                // Background image
                .child(
                    img("crates/blinc_app/examples/assets/stormy-plains-illuminated-stockcake.webp")
                        .size(800.0, 550.0)
                        .cover()
                        .rounded(16.0),
                )
                // Glass overlay with rain drops flow shader
                .child(
                    div()
                        .id("glass-layer")
                        .w(800.0)
                        .h(550.0)
                        .absolute()
                        .top(0.0)
                        .left(0.0)
                        .foreground(),
                )
                // Label
                .child(
                    div()
                        .absolute()
                        .bottom(16.0)
                        .left(24.0)
                        .foreground()
                        .child(
                            text("Wet Glass")
                                .size(28.0)
                                .weight(FontWeight::Bold)
                                .color(Color::rgba(1.0, 1.0, 1.0, 0.85)),
                        )
                        .child(
                            text("light refraction through water drops + condensation fog")
                                .size(14.0)
                                .color(Color::rgba(1.0, 1.0, 1.0, 0.55)),
                        ),
                ),
        )
}
