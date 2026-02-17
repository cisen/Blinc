# Flow Shaders

Flow shaders let you write GPU fragment shaders using a high-level DAG (directed acyclic graph) that compiles to WGSL. They can be defined in CSS stylesheets or directly in Rust using the `flow!` macro.

## Quick Start

The fastest way to add a flow shader to an element:

```rust
use blinc_layout::flow;

let ripple = flow!(ripple, fragment, {
    input uv: builtin(uv);
    input time: builtin(time);
    node d = distance(uv, vec2(0.5, 0.5));
    node wave = sin(d * 20.0 - time * 4.0) * 0.5 + 0.5;
    output color = vec4(wave, wave, wave, 1.0);
});

div().flow(ripple).w(400.0).h(400.0)
```

The `flow!` macro produces a `FlowGraph` using Rust identifiers and primitives. Pass it directly to any element via `.flow()`.

## Anatomy of a Flow Shader

Every flow shader has a **name**, a **target** (always `fragment` for visual effects), and a body of declarations:

```
@flow <name> {
    target: fragment;

    input <name>: builtin(<variable>);    // Input declarations
    step <name>: <step-type> { ... };     // Semantic steps (high-level)
    node <name> = <expression>;           // Raw computation nodes
    chain <name>: <step> | <step> | ...;  // Piped step chains
    use <flow-name>;                      // Compose other flows
    output color = <expression>;          // Output declarations
}
```

Declarations can appear in any order, but each node can only reference inputs and earlier nodes (the graph must be acyclic).

## Builtin Variables

| Variable | Type | Description |
|----------|------|-------------|
| `uv` | `vec2` | Normalized element coordinates (0,0 = top-left, 1,1 = bottom-right) |
| `time` | `float` | Elapsed time in seconds (monotonic) |
| `resolution` | `vec2` | Element size in physical pixels |
| `pointer` | `vec2` | Cursor position relative to element (0-1 range) |
| `sdf` | `float` | Signed distance field value at the current fragment |
| `frame_index` | `float` | Current frame number |

## Expressions

Flow expressions support standard arithmetic, vector constructors, function calls, and swizzle access:

```
node a = sin(uv.x * 10.0 + time);
node b = vec4(a, a * 0.5, 1.0 - a, 1.0);
node c = mix(b, vec4(1.0, 0.0, 0.0, 1.0), 0.5);
node d = c.rgb;
```

### Operators

| Operator | Example |
|----------|---------|
| `+`, `-`, `*`, `/` | `a * 2.0 + b` |
| Unary `-` | `-a` |
| Swizzle | `v.xy`, `v.rgb`, `v.x` |

### Functions Reference

**Math (scalar)**

| Function | Signature | Description |
|----------|-----------|-------------|
| `sin`, `cos`, `tan` | `f32 -> f32` | Trigonometric |
| `abs`, `floor`, `ceil`, `fract` | `f32 -> f32` | Rounding / absolute |
| `sqrt`, `exp`, `log`, `sign` | `f32 -> f32` | Algebraic |
| `pow` | `(f32, f32) -> f32` | Power |
| `atan2` | `(f32, f32) -> f32` | Arc tangent |
| `mod` | `(f32, f32) -> f32` | Modulus |
| `min`, `max` | `(f32, f32) -> f32` | Comparative |
| `clamp` | `(f32, f32, f32) -> f32` | Clamp to range |
| `mix` | `(f32, f32, f32) -> f32` | Linear interpolation |
| `smoothstep` | `(f32, f32, f32) -> f32` | Smooth Hermite |
| `step` | `(f32, f32) -> f32` | Step function |

**Vector**

| Function | Description |
|----------|-------------|
| `length(v)` | Vector magnitude |
| `distance(a, b)` | Distance between two points |
| `dot(a, b)` | Dot product |
| `cross(a, b)` | Cross product (vec3) |
| `normalize(v)` | Unit vector |
| `reflect(v, n)` | Reflection |

**Noise**

| Function | Signature | Description |
|----------|-----------|-------------|
| `fbm(p, octaves)` | `(vec2, i32) -> f32` | Fractal Brownian motion |
| `fbm_ex(p, octaves, persistence)` | `(vec2, i32, f32) -> f32` | FBM with custom persistence |
| `worley(p)` | `vec2 -> f32` | Worley/cellular noise |
| `worley_grad(p)` | `vec2 -> vec3` | Worley with analytic gradient (x=dist, y=gx, z=gy) |
| `checkerboard(p, scale)` | `(vec2, f32) -> f32` | Checkerboard pattern |

**SDF Primitives**

| Function | Description |
|----------|-------------|
| `sdf_box(p, half_size)` | Box SDF |
| `sdf_circle(p, radius)` | Circle SDF |
| `sdf_ellipse(p, radii)` | Ellipse SDF |
| `sdf_round_rect(p, half_size, radius)` | Rounded rectangle SDF |

**SDF Combinators**

| Function | Description |
|----------|-------------|
| `sdf_union(a, b)` | Union of two SDFs |
| `sdf_intersect(a, b)` | Intersection |
| `sdf_subtract(a, b)` | Subtraction |
| `sdf_smooth_union(a, b, k)` | Smooth union with radius k |
| `sdf_smooth_intersect(a, b, k)` | Smooth intersection |
| `sdf_smooth_subtract(a, b, k)` | Smooth subtraction |

**Lighting**

| Function | Description |
|----------|-------------|
| `phong(normal, light_dir, view_dir, shininess)` | Phong shading |
| `blinn_phong(normal, light_dir, view_dir, shininess)` | Blinn-Phong shading |

**Scene**

| Function | Description |
|----------|-------------|
| `sample_scene(uv)` | Sample the background behind this element (for refraction/glass effects) |

## Semantic Steps

Steps are high-level operations that expand to multiple nodes automatically. They provide a more declarative way to build shader effects.

### Pattern Steps

Generate procedural textures. Output type: `float` (scalar field).

| Step Type | Key Parameters | Description |
|-----------|---------------|-------------|
| `pattern_noise` | `scale`, `detail`, `animation` | FBM noise pattern |
| `pattern_worley` | `scale`, `threshold`, `edge`, `mask`, `gradient` | Worley cellular pattern with analytic gradient |
| `pattern_ripple` | `center`, `density`, `speed` | Concentric ripple rings |
| `pattern_waves` | `direction`, `frequency`, `speed` | Directional sine waves |
| `pattern_grid` | `scale`, `line_width` | Grid lines |
| `pattern_gradient` | `direction`, `start`, `end` | Linear gradient (output: vec4) |
| `pattern_plasma` | `scale`, `speed` | Plasma texture (output: vec4) |

### Effect Steps

Post-processing effects that modify appearance.

| Step Type | Key Parameters | Description |
|-----------|---------------|-------------|
| `effect_refract` | `source`, `strength` | Lens refraction via Worley gradient |
| `effect_frost` | `source`, `strength`, `detail` | Frosted glass UV jitter |
| `effect_specular` | `source`, `intensity`, `power` | Specular highlight scattering |
| `effect_fog` | `density`, `source` | Fog/haze composite |
| `effect_light` | `source`, `direction`, `intensity`, `power` | Directional highlights from normals |

### Transform Steps

Spatial coordinate transformations. Output type: `vec2` (UV coordinate) or `float`.

| Step Type | Key Parameters | Description |
|-----------|---------------|-------------|
| `transform_wet` | `aspect`, `scroll_speed`, `offset` | Aspect-corrected gravity scroll (for rain/drip effects) |
| `transform_warp` | `source`, `amount` | Warp UV by a noise field |
| `transform_rotate` | `angle` | Rotate UV coordinates |
| `transform_scale` | `factor` | Scale UV coordinates |
| `transform_tile` | `count` | Tile/repeat UV |
| `transform_mirror` | `axis` | Mirror UV |
| `transform_polar` | `center` | Cartesian to polar coordinates |

### Color Steps

Map scalar values to colors. Output type: `vec4`.

| Step Type | Key Parameters | Description |
|-----------|---------------|-------------|
| `color_ramp` | `source`, `stops`, `opacity` | Map scalar to color gradient |
| `color_shift` | `source`, `hue` | Hue shift |
| `color_tint` | `source`, `color` | Color tinting |
| `color_invert` | `source` | Color inversion |

### Composition Steps

Combine two sources. Output type: `vec4`.

| Step Type | Key Parameters | Description |
|-----------|---------------|-------------|
| `compose_blend` | `a`, `b`, `mode` | Blend two layers (screen, multiply, overlay, etc.) |
| `compose_mask` | `source`, `mask` | Alpha mask one input by another |
| `compose_layer` | `base`, `overlay`, `opacity` | Stack with opacity |

### Adjust Steps

Value curve shaping. Output type: `float`.

| Step Type | Key Parameters | Description |
|-----------|---------------|-------------|
| `adjust_falloff` | `radius`, `center` | Distance-based fade |
| `adjust_remap` | `source`, `in_min`, `in_max`, `out_min`, `out_max` | Remap value range |
| `adjust_threshold` | `source`, `value` | Hard threshold |
| `adjust_ease` | `source`, `curve` | Apply easing curve |
| `adjust_clamp` | `source`, `min`, `max` | Clamp value range |

## Chains

Chains pipe the output of one step into the next, creating a processing pipeline:

```
chain effect:
    pattern_ripple(center: vec2(0.5, 0.5), density: 25.0)
    | adjust_falloff(radius: 0.5)
    ;
```

Each link in the chain implicitly receives the previous link's output as its `source` parameter.

## Flow Composition with `use`

Flows can import nodes from other flows using `use`:

```
@flow base_noise {
    target: fragment;
    input uv: builtin(uv);
    node n = fbm(uv * 4.0, 6);
    output color = vec4(n, n, n, 1.0);
}

@flow enhanced {
    target: fragment;
    use base_noise;
    node bright = smoothstep(0.3, 0.7, n);
    output color = vec4(bright, bright * 0.5, 0.1, 1.0);
}
```

The `use` directive imports all nodes from the referenced flow into the current graph.

## Scene Sampling

For glass, refraction, or frosted effects, use `sample_scene()` to read the rendered background behind the element:

```rust
let glass = flow!(glass, fragment, {
    input uv: builtin(uv);
    input time: builtin(time);
    node offset = fbm(uv * 8.0 + vec2(time * 0.1, 0.0), 3) * 0.02;
    node scene = sample_scene(uv + vec2(offset, offset));
    output color = scene;
});
```

The scene texture is automatically captured before flow rendering begins. Elements using `sample_scene()` see everything rendered behind them.

## Applying Flow Shaders

There are three ways to apply flow shaders to elements:

### 1. `flow!` Macro (Recommended)

Define the shader in Rust and pass it directly to the element:

```rust
use blinc_layout::flow;

let shader = flow!(my_effect, fragment, {
    input uv: builtin(uv);
    input time: builtin(time);
    node wave = sin(uv.x * 10.0 + time) * 0.5 + 0.5;
    output color = vec4(wave, 0.2, 0.5, 1.0);
});

div().flow(shader).w(300.0).h(300.0)
```

The `FlowGraph` carries its own name and is auto-persisted by the GPU pipeline cache.

### 2. CSS Stylesheet

Define flows in CSS and reference them by name:

```rust
ctx.add_css(r#"
    @flow terrain {
        target: fragment;
        input uv: builtin(uv);
        step noise: pattern-noise { scale: 4.0; detail: 6; };
        output color = vec4(noise, noise, noise, 1.0);
    }

    #my-element {
        flow: terrain;
        border-radius: 16px;
    }
"#);

div().id("my-element").w(300.0).h(300.0)
```

### 3. Style Macros

Reference CSS-defined flows from `css!` or `style!` macros:

```rust
let style = css! {
    flow: "terrain";
    border-radius: 16px;
};

// Or with style! macro:
let style = style! {
    flow: "terrain",
    corner_radius: 16.0,
};
```

### 4. Name Reference

Reference a previously-defined flow by name string:

```rust
div().flow("terrain").w(300.0).h(300.0)
```

## Complete Example

Here's the wet glass demo that creates a realistic rain-on-glass effect using semantic steps:

```rust
use blinc_layout::flow;

let wetglass = flow!(wetglass, fragment, {
    input uv: builtin(uv);
    input time: builtin(time);
    input resolution: builtin(resolution);

    // Gravity gradient: more moisture at bottom
    node grav = smoothstep(0.0, 1.0, uv.y);

    // Background mist
    step mist: pattern_noise { scale: 3.0; detail: 5; animation: time * 0.02; };
    node moist = mist * (0.35 + grav * 0.65);

    // Multi-scale water drops with aspect correction and gravity scroll
    step uv1: transform_wet { aspect: resolution; scroll_speed: 0.001; };
    step uv2: transform_wet { aspect: resolution; scroll_speed: 0.0015; offset: vec2(0.38, 0.21); };
    step uv3: transform_wet { aspect: resolution; scroll_speed: 0.002; offset: vec2(0.17, 0.63); };

    // Worley drops at different scales
    step drops1: pattern_worley { uv: uv1; scale: 7.0; threshold: 0.22; edge: 0.05; mask: step(0.3, moist); gradient: true; };
    step drops2: pattern_worley { uv: uv2; scale: 12.0; threshold: 0.18; edge: 0.04; mask: step(0.2, moist); gradient: true; };
    step drops3: pattern_worley { uv: uv3; scale: 20.0; threshold: 0.13; edge: 0.03; mask: step(0.12, moist); gradient: true; };

    // Combine drops
    node drops_raw = clamp(drops1 + drops2 * 0.6 + drops3 * 0.3, 0.0, 1.0);
    node drops = smoothstep(0.05, 0.4, drops_raw);

    // Specular highlights
    step highlight: effect_specular {
        sources: drops1 drops2 drops3;
        weights: 1.0 0.6 0.3;
        direction: vec2(0.7071068, 0.7071067);
        intensity: 0.25;
        power: 64.0;
    };

    // Fog and lens distortion
    node fog = (1.0 - drops) * (0.12 + mist * 0.05);
    step lens: effect_refract { source: drops; strength: 0.025; };

    // Sample background scene through distorted UVs
    node scene = sample_scene(uv + lens);

    // Composite
    node out_r = scene.x * (1.0 - fog) + fog + highlight;
    node out_g = scene.y * (1.0 - fog) + fog + highlight;
    node out_b = scene.z * (1.0 - fog) + fog + highlight;
    output color = vec4(out_r, out_g, out_b, 0.97);
});

div().flow(wetglass).w(800.0).h(600.0)
```

## Performance Tips

- **Analytic gradients**: `pattern_worley` with `gradient: true` uses `worley_grad()` which computes distance + gradient in a single 3x3 grid pass (5x faster than finite-difference).
- **Pipeline caching**: Compiled WGSL pipelines are cached by flow name in `FlowPipelineCache`. Reusing the same flow name across frames is free after first compile.
- **Scene copy**: `sample_scene()` triggers a single texture copy per frame (not per element). Multiple elements sharing a scene-sampling flow share the same copy.
- **Step expansion**: Semantic steps expand to optimized node graphs at parse time, not at render time. There's zero per-frame overhead from using steps vs raw nodes.
