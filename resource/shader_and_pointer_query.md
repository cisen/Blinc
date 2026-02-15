# BLINC — Declarative Shader & Pointer Query System

**Technical Design Plan**

Version 1.0 · February 2026
Project Blinc · github.com/project-blinc/blinc
Author: Damilare Darmie Akinlaja

---

## Table of Contents

1. Executive Summary
2. System Overview & Architecture
3. Declarative Shader System — CSS-Driven GPU Rendering
   - 3.1 Property Taxonomy
   - 3.2 Shape & SDF Composition
   - 3.3 Surface & Material Model
   - 3.4 Physics Simulation Binding
   - 3.5 Effect Presets via @effect
   - 3.6 Compositing & Output
   - 3.7 Escape Hatch: Inline WGSL
4. Pointer Query System — @pointer & Continuous Interaction
   - 4.1 pointer-space Declaration
   - 4.2 Pointer Environment Variables
   - 4.3 Continuous Mode: calc() Binding
   - 4.4 Discrete Mode: @pointer Query Blocks
   - 4.5 Math Functions for Ergonomics
5. Integration: Shader × Pointer × Layout
6. Shader IR & Compilation Pipeline
7. Compute ↔ Render Resource Binding
8. Phased Implementation Plan
9. Open Questions & Future Work

---

## 1. Executive Summary

This document defines the technical design for two tightly coupled systems within the Blinc rendering framework: a declarative shader system driven by CSS-like property declarations, and a pointer query system that exposes continuous interaction data as live environment variables. Together, these systems enable GPU-accelerated visual effects — from SDF rendering and physically-based materials to fluid simulation and cloth dynamics — to be authored using familiar, natural CSS syntax, without requiring developers to write shader code.

The core thesis is that CSS properties should function as the frontend syntax for a shader intermediate representation (IR), which compiles to GPU pipelines. Pointer queries provide the interaction primitive that feeds continuous user input into these pipelines, closing the loop between input, simulation, and rendering.

> **Design Principle:** Every visual effect expressible as a shader should be declarable through CSS properties that feel native to the language. No `shader-*` prefixes. No foreign syntax. Properties like `viscosity`, `surface-roughness`, and `gravity` are first-class CSS values that flow through `calc()`, `transition`, and `@keyframes` like any other property.

---

## 2. System Overview & Architecture

The system is structured as a layered compilation pipeline. CSS declarations are parsed into a typed Shader IR, which is then compiled into WGSL shader modules and bound to GPU render and compute pipelines.

```
CSS Declarations + @pointer + @effect
    │ parse
Shader IR (typed graph: SDF ops, uniforms, sim configs, interactions)
    │ compile
WGSL Modules (fragment shaders, compute kernels)
    │ bind
GPU Pipelines (render pass + compute pass + resource transitions)
    │ execute
Frame Output (composited with Blinc layout)
```

The architecture separates into three conceptual layers:

- **Declaration Layer:** CSS properties, `@effect` blocks, `@pointer` queries, and `calc()` expressions define the intent. This is the developer-facing API.
- **IR Layer:** A typed expression graph representing SDF operations, material parameters, simulation configurations, interaction bindings, and buffer dependencies. This is where composition and optimization happen.
- **Execution Layer:** WGSL shaders, compute dispatches, buffer allocations, and resource transitions. This is where GPU work is scheduled and the Blinc compute-to-render binding framework manages synchronization.

---

## 3. Declarative Shader System

The shader system extends CSS with new properties and value functions that map directly to GPU concepts. The guiding rule: if CSS already has a concept for it, extend that concept. Only introduce new property names when no existing CSS concept applies, and when the name is a natural domain word (not a prefixed namespace).

### 3.1 Property Taxonomy

All shader-related CSS properties fall into six categories. Each category maps to a distinct part of the GPU pipeline.

| Category | CSS Properties | GPU Mapping |
|---|---|---|
| Shape / SDF | `shape`, `shape-combine`, `shape-mask`, `shape-mask-mode` | SDF evaluation in fragment shader |
| Surface / Material | `surface`, `surface-color`, `surface-roughness`, `surface-normal`, `surface-fresnel` | Material uniforms + shading model selection |
| Lighting | `light`, `light-intensity`, `light-color`, `ambient` | Light uniforms in fragment shader |
| Physics / Simulation | `physics`, `physics-resolution`, `viscosity`, `stiffness`, `damping`, `gravity`, `amplitude`, `turbulence`, `speed`, `surface-tension` | Compute pipeline configuration + uniforms |
| Displacement | `displace`, `displace-scale` | Buffer read in fragment shader (sim → render binding) |
| Compositing | `blend`, `render`, `refract`, `refract-chromatic` | Blend state + post-processing passes |

### 3.2 Shape & SDF Composition

The `shape` property defines the SDF primitive for the element. It accepts one or more SDF functions, mirroring how CSS `background` accepts multiple layers. When multiple shapes are declared, they are combined using the `shape-combine` property.

**Primitive Functions:**

```css
shape: rect(width, height, round: <radius>);
shape: circle(radius);
shape: ellipse(rx, ry);
shape: polygon(<points>);
shape: path(<svg-path-data>);
```

**CSG Composition:**

```css
/* Multiple shapes composed via shape-combine */
.blob {
  shape:
    circle(40% at 30% 50%),
    circle(40% at 70% 50%);
  shape-combine: smooth-union(8px);
}

/* Subtraction via mask (reuses CSS mask concept) */
.cutout {
  shape: rect(100%, 100%, round: 12px);
  shape-mask: circle(20% at 50% 50%);
  shape-mask-mode: subtract;
}
```

The `shape-combine` property accepts: `union`, `smooth-union(radius)`, `intersect`, `smooth-intersect(radius)`, `subtract`, and `smooth-subtract(radius)`. The smooth variants use polynomial smooth-min/max in the SDF evaluation, producing organic blends between primitives.

### 3.3 Surface & Material Model

The `surface` property selects a shading model. Sub-properties configure material parameters. All surface properties are animatable and accept `calc()` expressions, enabling pointer-driven or time-driven material changes.

| Property | Values | Description |
|---|---|---|
| `surface` | `flat` \| `matte` \| `glossy` \| `phong` \| `pbr` | Selects the shading model used in the fragment shader |
| `surface-color` | Any CSS color or gradient | Base albedo color; gradients map across UV space |
| `surface-roughness` | `0.0` – `1.0` | Microsurface roughness; affects specular spread |
| `surface-normal` | `none` \| `from-displacement` \| `noise(...)` | Normal source: computed from heightfield or procedural |
| `surface-fresnel` | `0.0` – `1.0` | Fresnel intensity at grazing angles |

```css
.card {
  surface: glossy;
  surface-color: linear-gradient(135deg, #3a1c71, #d76d77);
  surface-roughness: 0.35;
  surface-fresnel: 0.6;
  surface-normal: from-displacement;

  light: 0.3 -0.8 0.5;     /* direction vector */
  light-intensity: 1.2;
  light-color: #ffffff;
  ambient: 0.15;
}
```

### 3.4 Physics Simulation Binding

The `physics` property activates a compute pipeline for the element. When set, the Blinc compiler allocates simulation buffers (position, velocity, force accumulators) and generates compute shader dispatches that run before the element's render pass each frame.

**Simulation Types:**

| Type | Kernel | Key Parameters |
|---|---|---|
| `physics: cloth` | Mass-spring grid solver | `stiffness`, `damping`, `gravity`, `physics-resolution` |
| `physics: fluid` | 2D Navier-Stokes / shallow water | `viscosity`, `surface-tension`, `speed`, `amplitude`, `turbulence` |
| `physics: particles` | Verlet integration particle system | `gravity`, `damping`, `emit-rate`, `lifetime` |
| `physics: spring` | Damped harmonic oscillator per-vertex | `stiffness`, `damping`, `mass` |
| `physics: wave` | Wave equation solver | `speed`, `damping`, `amplitude` |
| `physics: none` | No simulation (default) | — |

**Context-Dependent Properties:**

Parameters like `viscosity`, `stiffness`, and `gravity` are context-dependent: they only resolve when a physics type is declared on the same element. The CSS parser treats them as inert when `physics: none` (or undeclared). This mirrors how `animation-delay` is meaningless without `animation-name`. There is no ambiguity because these domain words have no alternate meaning in CSS.

```css
.fluid-card {
  physics: fluid;
  physics-resolution: 128 128;
  viscosity: 0.3;
  surface-tension: 0.05;
  speed: 0.8;

  /* Displacement links sim output to visual */
  displace: physics-velocity;
  displace-scale: 24px;
}
```

The `displace` property reads from a named simulation buffer and applies it as a deformation to the SDF. Available sources: `physics-position` (heightfield), `physics-velocity` (velocity field), `physics-normal` (derived normals). The `displace-scale` property controls the magnitude of displacement in element-local pixels.

### 3.5 Effect Presets via @effect

The `@effect` at-rule defines a reusable simulation configuration with named parameter presets. It mirrors `@keyframes` in structure: a named block containing states that can be referenced by elements. Unlike `@keyframes` (which are temporal), `@effect` presets are parameter states that can be interpolated based on any input — pointer position, time, scroll position, or any CSS variable.

```css
@effect fluid-surface {
  type: fluid;
  resolution: 128 128;

  /* Named parameter presets */
  still {
    viscosity: 0.95;
    amplitude: 0.02;
    turbulence: 0.0;
    depth-color: #c8d4e0;
  }

  storm {
    viscosity: 0.15;
    amplitude: 0.5;
    turbulence: 0.7;
    depth-color: #2a5a8a;
  }

  /* Computed outputs exposed as CSS variables */
  output: energy, peak-height, velocity;
}
```

Elements bind an effect with `effect: <name>`. The output values become live CSS environment variables scoped to the element (e.g., `energy`, `peak-height`), usable in `calc()` for shadows, opacity, color, or any other property. The `@effect` system is optional sugar — developers can always set physics parameters directly via individual properties and `calc()` expressions without defining an `@effect` block.

### 3.6 Compositing & Output

Shader output must composite back into the Blinc layout system. The following properties control how rendered shader output integrates with the element tree:

| Property | Values | Behavior |
|---|---|---|
| `blend` | `source-over` \| `multiply` \| `screen` \| `overlay` \| `add` | Compositing operation against backdrop |
| `render` | `replace` \| `overlay` \| `mask` | How shader output relates to element content |
| `refract` | `<ior>` | Index of refraction for distortion effects |
| `refract-chromatic` | `<amount>` | Chromatic aberration offset per RGB channel |

### 3.7 Escape Hatch: Inline WGSL

For effects beyond what declarative properties can express, developers can embed WGSL directly. The `fragment` property accepts a `wgsl()` function containing a shader function body. This provides full GPU programmability while remaining inside the CSS file:

```css
.custom-effect {
  shape: rect(100%, 100%);

  fragment: wgsl(
    fn shade(uv: vec2f, time: f32, sim: texture_2d<f32>) -> vec4f {
      let d = sdf_round_rect(uv, vec2f(1.0), 0.05);
      let vel = textureSample(sim, linear_sampler, uv).xy;
      return vec4f(vel * 0.5 + 0.5, 0.0, smoothstep(0.01, 0.0, d));
    }
  );
}
```

The compiler injects the function into a full WGSL module with pre-bound uniforms (`time`, `resolution`, pointer state) and simulation buffers. The developer only writes the core logic. Standard Blinc SDF helper functions (`sdf_round_rect`, `sdf_circle`, etc.) are available in scope.

---

## 4. Pointer Query System

The pointer query system exposes cursor/touch interaction as continuous, queryable environment variables scoped to individual elements. It is conceptually analogous to CSS media queries and environment variables (`env(safe-area-inset-top)`), but for interaction state rather than viewport state. The pointer position within an element's local coordinate space becomes a live signal that flows through `calc()`, `transition`, and the entire CSS property system.

### 4.1 pointer-space Declaration

An element opts into pointer querying by declaring a `pointer-space`. This registers the element as a pointer-query source, defining the coordinate system for all `pointer-*` variables on that element and its descendants.

| Property | Values | Behavior |
|---|---|---|
| `pointer-space` | `self` \| `parent` \| `nearest(<selector>)` | Which element defines the coordinate bounds |
| `pointer-origin` | `center` \| `top-left` \| `bottom-left` | Where (0,0) sits within the element |
| `pointer-range` | `<min> <max>` | Normalized output range, e.g. `-1 1` or `0 1` |
| `pointer-smoothing` | `<duration>` | Temporal smoothing applied to all pointer values |

```css
.interactive-card {
  pointer-space: self;
  pointer-origin: center;
  pointer-range: -1 1;
  pointer-smoothing: 0.15s;
}
```

### 4.2 Pointer Environment Variables

Once `pointer-space` is declared, the following environment variables become live on the element. These update every frame and are usable anywhere a CSS value is accepted, including within `calc()` expressions.

| Variable | Type | Description |
|---|---|---|
| `pointer-x` | float | Horizontal position in `pointer-range`, smoothed |
| `pointer-y` | float | Vertical position in `pointer-range`, smoothed |
| `pointer-vx` | float | Horizontal velocity (units/second) |
| `pointer-vy` | float | Vertical velocity (units/second) |
| `pointer-speed` | float | Magnitude of velocity vector |
| `pointer-dx` | float | Raw horizontal delta since last frame |
| `pointer-dy` | float | Raw vertical delta since last frame |
| `pointer-distance` | float | Distance from `pointer-origin` (0 at origin, 1 at farthest bound) |
| `pointer-angle` | angle | Angle from origin in CSS turn units |
| `pointer-pressure` | float | Device pressure (0–1), if available; fallback 0 or 1 |
| `pointer-inside` | 0 \| 1 | Whether pointer is within element bounds |
| `pointer-active` | 0 \| 1 | Whether pointer is pressed/touching |
| `pointer-duration` | float | Seconds since pointer entered or activated |

### 4.3 Continuous Mode: calc() Binding

Pointer variables flow directly into `calc()` expressions, enabling continuous mapping from interaction to any CSS property. This is the primary usage mode — the developer writes the transfer function that maps pointer state to visual output.

**Example: 3D Corner Tilt on Hover**

```css
.tilt-card {
  pointer-space: self;
  pointer-origin: center;
  pointer-range: -1 1;

  perspective: 800px;
  transform-style: preserve-3d;

  transform:
    rotateY(calc(pointer-x * 15deg))
    rotateX(calc(pointer-y * -15deg));

  transform-origin:
    calc(50% + pointer-x * 30%)
    calc(50% + pointer-y * 30%);

  shadow:
    calc(pointer-x * -12px)
    calc(pointer-y * 12px)
    32px rgba(0, 0, 0, 0.2);

  transition: transform 0.15s ease-out;
}
```

**Example: Physics Parameters Driven by Cursor Distance**

```css
.fluid-card {
  pointer-space: self;
  pointer-range: -1 1;

  physics: fluid;
  physics-resolution: 128 128;

  viscosity:  calc(0.9 - pointer-distance * 0.85);
  amplitude:  calc(0.02 + pointer-distance * 0.58);
  turbulence: calc(pointer-distance * pointer-distance * 0.9);

  surface-color: color-mix(
    in oklch, #c8d4e0, #1a3a5a calc(pointer-distance * 100%)
  );
}
```

The continuous mode entirely replaces the need for predefined interaction maps or named presets for most use cases. The developer has full control over the mapping function. Standard CSS transitions apply to the computed output values, providing temporal smoothing.

### 4.4 Discrete Mode: @pointer Query Blocks

For cases requiring discrete state changes rather than continuous interpolation, the `@pointer` at-rule provides conditional blocks analogous to `@media` queries. These enable region-based or state-based activation of properties.

```css
.card {
  pointer-space: self;
  pointer-range: -1 1;
  physics: none;
}

/* Activate simulation when pointer is beyond 30% from center */
@pointer (distance > 0.3) and (inside) {
  .card {
    physics: fluid;
    viscosity: calc(0.9 - pointer-distance * 0.85);
  }
}

/* Inject force on press */
@pointer (active) {
  .card {
    interact: pointer;
    interact-force: calc(12.0 + pointer-duration * 5.0);
  }
}

/* Regions: detect which quadrant the pointer is in */
@pointer (x > 0.5) and (y < -0.5) {
  .card .corner-indicator-tr { opacity: 1; }
}
```

The `@pointer` query supports the following conditions: `x`, `y` (comparison against position), `distance` (from origin), `angle` (in turns), `speed` (velocity magnitude), `active` (pressed), `inside` (within bounds), and `pressure`. Conditions combine with `and`/`or`/`not` operators, following the same syntax as `@media`.

> **Design Note: Two Access Patterns, One Primitive.** Continuous mode (`calc()` with `pointer-x`, `pointer-distance`, etc.) and discrete mode (`@pointer` blocks) are not separate systems. They share the same underlying `pointer-space` declaration and the same computed pointer variables. The developer chooses which access pattern fits their use case, and can mix both freely on the same element.

### 4.5 Math Functions for Ergonomics

Raw `calc()` expressions with pointer variables can become verbose for common interaction patterns. Blinc introduces a small set of math functions that compose naturally with `calc()` and reduce boilerplate:

| Function | Signature | Description |
|---|---|---|
| `mix()` | `mix(a, b, t)` | Linear interpolation: `a + (b - a) * t` |
| `remap()` | `remap(v, in_lo, in_hi, out_lo, out_hi)` | Remap value from one range to another |
| `smoothstep()` | `smoothstep(edge0, edge1, x)` | Smooth Hermite interpolation (0–1) |
| `deadzone()` | `deadzone(value, threshold)` | Returns 0 below threshold, scaled value above |
| `spring()` | `spring(target, stiffness, damping)` | Physics-based easing toward target value |
| `clamp()` | `clamp(min, value, max)` | Clamp value to range (already in CSS) |
| `step()` | `step(edge, x)` | Returns 0 if x < edge, 1 otherwise |

**Ergonomic Example:**

```css
.fluid-card {
  /* Without helpers: */
  viscosity: calc(0.9 + (0.05 - 0.9) * pointer-distance);

  /* With mix(): */
  viscosity: mix(0.9, 0.05, pointer-distance);

  /* Smooth ramp with deadzone: */
  turbulence: smoothstep(0.2, 0.8, deadzone(pointer-speed, 50) * 0.01);

  /* Physics-based spring on transform: */
  transform: rotateY(spring(pointer-x * 20deg, 300, 15));
}
```

---

## 5. Integration: Shader × Pointer × Layout

The shader and pointer systems are designed to compose with Blinc's existing layout engine. The layout engine owns coordinate mapping: SDF space is always `[0,1]` UV within the element bounds, and pointer coordinates are normalized within the declared `pointer-space`. The layout engine provides the transform from screen space to element-local space, keeping shader code resolution-independent.

**Interaction with CSS Pseudo-classes**

Standard CSS pseudo-classes work naturally with both systems. The physics and pointer properties are regular CSS properties, so `:hover`, `:active`, `:focus`, and `:target` all apply. Combined with `transition`, this enables state-driven effects:

```css
.surface {
  physics: none;
  transition: displace-scale 0.4s ease-out, viscosity 0.3s;
}

.surface:hover {
  physics: fluid;
  viscosity: calc(0.9 - pointer-distance * 0.85);
}

.surface:active {
  interact: pointer;
  interact-force: 12.0;
}

.surface:not(:hover) {
  transition: displace-scale 0.6s ease-out;  /* slower settle */
}
```

**Interaction with CSS @keyframes**

Physics parameters can be animated with `@keyframes`, enabling time-driven effects that layer on top of pointer-driven interaction:

```css
@keyframes breathe {
  0%, 100% { amplitude: 0.05; speed: 0.2; }
  50%      { amplitude: 0.15; speed: 0.5; }
}

.ambient-surface {
  physics: fluid;
  animation: breathe 4s ease-in-out infinite;
  /* Pointer interaction adds on top of animated base */
  viscosity: calc(0.5 - pointer-distance * 0.3);
}
```

**Child Element Awareness**

Simulation output variables (`energy`, `peak-height`, `velocity`) from `@effect` declarations are scoped to the element and inherited by descendants. This allows child elements — text labels, icons, overlays — to react to the simulation state of their parent:

```css
.fluid-card .label {
  opacity: calc(1.0 - energy * 0.5);
  filter: blur(calc(energy * 2px));
  transform: translateY(calc(peak-height * -4px));
}
```

---

## 6. Shader IR & Compilation Pipeline

The Shader IR is the central data structure that bridges CSS declarations and GPU pipelines. Each element with shader-related properties produces an IR graph. The compiler walks this graph to emit WGSL and configure pipeline state.

**IR Node Types:**

```rust
enum ShaderNode {
    // SDF primitives
    SdfRect { size: Vec2, radius: f32 },
    SdfCircle { radius: f32 },
    SdfPath { segments: Vec<PathSeg> },

    // SDF combinators
    SdfUnion { a: Box<ShaderNode>, b: Box<ShaderNode>, smooth_k: f32 },
    SdfSubtract { a: Box<ShaderNode>, b: Box<ShaderNode> },
    SdfIntersect { a: Box<ShaderNode>, b: Box<ShaderNode> },

    // Deformers (read from simulation buffers)
    Displace { source: Box<ShaderNode>, buffer: SimBufferRef, scale: f32 },
    Warp { source: Box<ShaderNode>, func: WarpFunc },

    // Materials and lighting
    Material { model: ShadingModel, params: MaterialParams },
    Light { direction: Vec3, intensity: f32, color: Vec3 },

    // Simulation (maps to compute pipeline)
    Sim(SimConfig),

    // Interaction (pointer force injection)
    Interact(InteractConfig),
}
```

**Compilation Strategy: Two Phases**

**Phase 1 — Template Shaders with Uniform Specialization.** Pre-authored WGSL templates for each simulation type and material model. The CSS compiler fills in uniform values and toggles conditional branches via specialization constants. Fast iteration, limited expressiveness. Sufficient for all standard properties.

**Phase 2 — Full IR-to-WGSL Codegen.** The Shader IR compiles to WGSL from scratch, enabling arbitrary SDF composition and novel combinations. This is where smooth-union between a cloth-simulated surface and a fluid surface becomes possible — the compiler generates combined fragment shaders that evaluate multiple SDF operations and blend results.

---

## 7. Compute ↔ Render Resource Binding

Physics simulations run as compute passes that produce buffers consumed by the render pass. The CSS declaration model makes these bindings implicit — when the developer writes `displace: physics-velocity`, the compiler knows to allocate, synchronize, and bind the velocity buffer across pipeline stages.

**Per-Frame Execution Order:**

```
1. Interaction Injection (compute)
   │ Reads: pointer position, force params
   │ Writes: force accumulator buffer
   ↓
2. Simulation Step (compute, N iterations)
   │ Reads: force buffer, previous state
   │ Writes: position buffer, velocity buffer, normal buffer
   │ Writes: energy/metadata readback buffer
   ↓
3. Resource Transition (barrier)
   │ Storage → Read-only texture/buffer
   ↓
4. Render Pass (fragment shader)
   │ Reads: position buffer (displacement)
   │ Reads: velocity buffer (flow visualization, motion blur)
   │ Reads: normal buffer (lighting)
   │ Evaluates: SDF + material + compositing
   ↓
5. Compositing
   │ Blends shader output with layout backdrop
   ↓
6. Readback
     CSS engine reads energy, peak-height for use in next frame
```

The key design constraint: simulation and rendering are always separate pipelines connected by buffers. Physics is never evaluated inside the fragment shader. This keeps frame timing predictable, enables the simulation to run at a different resolution than the render target, and preserves composability (multiple elements can share or reference each other's simulation outputs in future extensions).

---

## 8. Phased Implementation Plan

### Phase 1: Foundation

**Goal:** Static SDF rendering and basic surface materials via CSS properties. No simulation or pointer queries.

- Implement `shape` property parser with `rect`, `circle`, `ellipse` primitives
- Implement `surface`, `surface-color`, `surface-roughness` as uniform bindings
- Implement `light`, `ambient` as uniform bindings
- Build template-based WGSL codegen for `flat`, `matte`, `glossy`, `phong` shading models
- Implement SDF fragment shader pipeline integrated with Blinc layout
- Implement `blend` and `render` compositing modes

**Deliverable:** Any Blinc layout element can be rendered as an SDF shape with material shading, driven entirely by CSS declarations.

### Phase 2: Pointer Queries

**Goal:** Continuous pointer interaction driving CSS properties via `calc()`.

- Implement `pointer-space`, `pointer-origin`, `pointer-range` property parsing
- Implement pointer variable computation from Blinc input system (screen → element-local transform)
- Expose `pointer-x`, `pointer-y`, `pointer-distance`, `pointer-angle`, `pointer-speed`, `pointer-active`, `pointer-inside` as live CSS environment variables
- Implement `pointer-smoothing` via exponential decay filter
- Enable pointer variables in `calc()` expressions for all existing CSS properties (`transform`, `shadow`, `opacity`, etc.) and all Phase 1 shader properties
- Implement `@pointer` conditional query blocks

**Deliverable:** 3D tilt cards, pointer-tracked materials, region-based activation — all via CSS.

### Phase 3: Physics Simulation

**Goal:** Compute-based simulation bound to elements via CSS.

- Implement `physics` property and simulation type registry (`cloth`, `fluid`, `wave`)
- Build compute shader templates for each simulation type
- Implement simulation buffer allocation and compute-to-render resource binding
- Implement `displace`, `displace-scale` as fragment shader buffer reads
- Implement `surface-normal: from-displacement` (Sobel filter on heightfield)
- Implement `interact: pointer` for force injection from pointer position
- Implement physics parameter animation via `transition` and `@keyframes`

**Deliverable:** The fluid surface effect from the reference video, fully controlled by pointer position via CSS declarations.

### Phase 4: Composition & Polish

**Goal:** Full SDF CSG, `@effect` presets, simulation output variables, and math functions.

- Implement SDF CSG: `shape-combine` (`smooth-union`, `intersect`, `subtract`), `shape-mask`
- Implement `@effect` at-rule with named presets and output variable declarations
- Implement simulation output readback (`energy`, `peak-height`, `velocity`) as scoped CSS variables
- Implement math helper functions: `mix()`, `remap()`, `smoothstep()`, `deadzone()`, `spring()`, `step()`
- Implement `refract` and `refract-chromatic` post-processing
- Begin Phase 2 IR-to-WGSL codegen for arbitrary SDF composition
- Implement `fragment: wgsl()` escape hatch

**Deliverable:** Complete system as specified. Any shader effect expressible through CSS or inline WGSL, with continuous pointer-driven interaction.

---

## 9. Open Questions & Future Work

**Multi-element simulation coupling.** Should adjacent elements be able to share a simulation domain? For example, two `fluid-card` elements whose fluid surfaces interact at their boundary. This would require a shared buffer allocation model and changes to the per-element isolation assumption.

**Scroll-driven interaction.** The pointer query system naturally extends to scroll position. A `scroll-space` property (analogous to `pointer-space`) would expose `scroll-x`, `scroll-y`, `scroll-velocity` etc. This is a clean future addition that reuses the same `calc()`-binding architecture.

**GPU readback latency.** Simulation output variables (`energy`, `peak-height`) require GPU → CPU readback, which introduces 1–2 frames of latency. For shadow and opacity animation this is acceptable, but for layout-affecting properties it may cause visible desync. The recommended constraint is that simulation outputs should only drive visual properties, not layout-affecting properties (`width`, `height`, `margin`, `padding`).

**Performance budgeting.** How should the system behave when multiple elements declare physics simulations and the GPU budget is exceeded? Options include automatic quality scaling (reducing `physics-resolution`), prioritization by visibility/screen-area, or developer-specified budgets via a `performance` CSS property.

**Accessibility.** Pointer-driven effects must degrade gracefully. A `prefers-reduced-motion` media query should disable physics simulations and pointer-driven transforms by default. Elements should remain fully functional without shader effects. The shader layer is visual enhancement, never structural.

**Collaboration with standard CSS.** If Blinc ever targets web rendering, the custom property names must not collide with future CSS specifications. A CSS Houdini `@property` registration strategy should be defined to namespace Blinc properties in browser contexts while keeping them prefix-free in native Blinc rendering.

---

*End of document.*