# Pointer Query

The pointer query system exposes continuous cursor position, velocity, distance, and angle as CSS environment variables on any element. This lets you build pointer-reactive effects — 3D tilt, hover reveals, distance-based glow, dynamic corners — entirely in CSS, with no Rust event handlers.

## How It Works

1. Set `pointer-space` on an element in CSS to enable tracking.
2. Each frame, the system computes the pointer's normalized position relative to that element.
3. Results are exposed as `env()` variables usable in any `calc()` expression.
4. Any numerical CSS property can read these values: opacity, border-radius, rotate, border-width, perspective transforms, and more.

```css
#card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.08;

    /* 3D tilt follows cursor */
    perspective: 800px;
    rotate-y: calc(env(pointer-x) * env(pointer-inside) * 25deg);
    rotate-x: calc(env(pointer-y) * env(pointer-inside) * -25deg);
}
```

```rust
div()
    .id("card")
    .class("my-card")
    .w(300.0)
    .h(200.0)
    .child(text("Hover me"))
```

No event handlers, no state management — the CSS drives everything.

---

## CSS Properties

These properties configure pointer tracking on an element. Setting `pointer-space` activates the system for that element.

### `pointer-space`

The coordinate space for pointer position computation.

| Value | Description |
| --- | --- |
| `self` | Position relative to the element's own bounds (default) |
| `parent` | Position relative to the parent element |
| `viewport` | Position relative to the viewport |

```css
#card { pointer-space: self; }
```

### `pointer-origin`

The origin point for coordinate normalization.

| Value | Description |
| --- | --- |
| `center` | (0,0) at element center, extends symmetrically (default) |
| `top-left` | (0,0) at top-left corner |
| `bottom-left` | (0,0) at bottom-left, Y-up (shader coordinates) |

```css
#card { pointer-origin: center; }
```

### `pointer-range`

The output range for normalized coordinates. Takes two floats: min and max.

```css
/* Default: symmetric -1 to 1 (good for center origin) */
#card { pointer-range: -1.0 1.0; }

/* 0 to 1 (good for top-left origin) */
#card { pointer-range: 0.0 1.0; }
```

With `center` origin and `-1.0 1.0` range:

- Cursor at element center: `pointer-x = 0`, `pointer-y = 0`
- Cursor at left edge: `pointer-x = -1`
- Cursor at right edge: `pointer-x = 1`

### `pointer-smoothing`

Exponential smoothing time constant in seconds. Smooths position, velocity, and the `pointer-inside` flag for gradual transitions.

```css
/* No smoothing — instant tracking */
#card { pointer-smoothing: 0; }

/* Subtle lag — responsive but smooth */
#card { pointer-smoothing: 0.08; }

/* Heavy smoothing — slow, floaty feel */
#card { pointer-smoothing: 0.2; }
```

When the cursor leaves the element, smoothed values decay toward the origin (0,0) instead of snapping. This creates a natural fade-out effect.

---

## Environment Variables

Once `pointer-space` is set on an element, these `env()` variables resolve inside any `calc()` expression on that element:

| Variable | Type | Description |
| --- | --- | --- |
| `env(pointer-x)` | float | Normalized X position in configured range |
| `env(pointer-y)` | float | Normalized Y position in configured range |
| `env(pointer-vx)` | float | X velocity (normalized units/second) |
| `env(pointer-vy)` | float | Y velocity (normalized units/second) |
| `env(pointer-speed)` | float | Total speed: `sqrt(vx² + vy²)` |
| `env(pointer-distance)` | float | Distance from origin (normalized units) |
| `env(pointer-angle)` | float | Angle from origin (radians, 0 = right, pi/2 = up) |
| `env(pointer-inside)` | 0.0/1.0 | 1.0 if cursor is inside element, 0.0 otherwise (smoothed) |
| `env(pointer-active)` | 0.0/1.0 | 1.0 if mouse button is pressed while over element |
| `env(pointer-hover-duration)` | float | Seconds since cursor entered (0 if outside) |

### Using `pointer-inside` as a Gate

Multiply by `env(pointer-inside)` to make effects only appear on hover:

```css
/* Rotation ONLY when hovered */
rotate: calc(env(pointer-x) * env(pointer-inside) * 5deg);

/* Opacity: 0.3 normally, 1.0 on hover */
opacity: calc(mix(0.3, 1.0, env(pointer-inside)));
```

Because `pointer-inside` is smoothed, the transition in/out is gradual when `pointer-smoothing` is set.

---

## Calc Functions

These functions work inside `calc()` and are especially useful with pointer variables:

| Function | Signature | Description |
| --- | --- | --- |
| `mix` | `mix(a, b, t)` | Linear interpolation: `a + (b - a) * t` |
| `smoothstep` | `smoothstep(edge0, edge1, x)` | Hermite interpolation (smooth 0-1 curve) |
| `step` | `step(edge, x)` | 0 if x < edge, 1 otherwise |
| `clamp` | `clamp(min, val, max)` | Clamp value to range |
| `remap` | `remap(val, in_lo, in_hi, out_lo, out_hi)` | Remap from one range to another |

### `mix` — Linear Interpolation

```css
/* Opacity: 30% when far, 100% when hovering */
opacity: calc(mix(0.3, 1.0, env(pointer-inside)));

/* Border-radius: 4px far, 48px near */
border-radius: calc(mix(4, 48, smoothstep(1.4, 0.0, env(pointer-distance))) * 1px);
```

### `smoothstep` — Smooth Transitions

Creates an S-curve between two edge values. When `edge0 > edge1`, the curve is inverted (1 at close range, 0 at far range).

```css
/* Opacity fades in as pointer approaches (inverted smoothstep) */
opacity: calc(smoothstep(1.8, 0.0, env(pointer-distance)));
```

### Units in `calc()`

Pointer env variables are unitless floats. To produce a CSS value with units, multiply by a unit literal:

```css
/* 1px unit applied after the math */
border-radius: calc(mix(4, 48, env(pointer-inside)) * 1px);
border-width: calc(mix(0, 4, env(pointer-inside)) * 1px);

/* Degrees for rotation */
rotate-y: calc(env(pointer-x) * 25deg);
```

---

## Examples

### 3D Tilt Card

Perspective rotate-x/y follow the cursor for a true 3D card effect.

```css
#tilt-card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.08;

    border-radius: 16px;
    background: #1e2438;
    perspective: 800px;
    rotate-y: calc(env(pointer-x) * env(pointer-inside) * 25deg);
    rotate-x: calc(env(pointer-y) * env(pointer-inside) * -25deg);
}
```

### Hover Reveal

Element fades from dim to full brightness on hover.

```css
#reveal-card {
    pointer-space: self;
    pointer-smoothing: 0.12;

    background: #2a1a3e;
    opacity: calc(mix(0.3, 1.0, env(pointer-inside)));
}
```

### Distance-Based Effects

Opacity, corners, or borders that respond to how close the cursor is to the element's center.

```css
#distance-card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.06;

    /* Opacity increases as pointer approaches center */
    opacity: calc(smoothstep(1.8, 0.0, env(pointer-distance)));
}

#corners-card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.08;

    /* Corners round as pointer approaches */
    border-radius: calc(mix(4, 48, smoothstep(1.4, 0.0, env(pointer-distance))) * 1px);
}
```

### Border Glow

Border grows and appears as the cursor approaches.

```css
#border-card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.06;

    border-radius: 16px;
    border-color: #4488cc;
    border-width: calc(mix(0, 4, smoothstep(1.4, 0.0, env(pointer-distance))) * 1px);
    opacity: calc(mix(0.3, 1.0, smoothstep(1.8, 0.0, env(pointer-distance))));
}
```

### Subtle Rotation

Card rotates gently following cursor x-position.

```css
#rotate-card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.1;

    rotate: calc(env(pointer-x) * env(pointer-inside) * 5deg);
    opacity: calc(mix(0.5, 1.0, env(pointer-inside)));
}
```

### Combined Effects

Multiple properties respond simultaneously for rich interactive cards.

```css
#combo-card {
    pointer-space: self;
    pointer-origin: center;
    pointer-range: -1.0 1.0;
    pointer-smoothing: 0.08;

    border-radius: calc(mix(8, 40, smoothstep(1.4, 0.0, env(pointer-distance))) * 1px);
    border-width: calc(mix(0, 3, smoothstep(1.2, 0.0, env(pointer-distance))) * 1px);
    border-color: #cc66aa;
    opacity: calc(smoothstep(1.6, 0.0, env(pointer-distance)));
    rotate: calc(env(pointer-x) * env(pointer-inside) * 3deg);
}
```

---

## How It Works Internally

1. **Registration**: When the CSS parser encounters `pointer-space` on an element, it stores a `PointerSpaceConfig` on the `ElementStyle`. During stylesheet application, elements with this config are registered in `PointerQueryState`.

2. **Per-frame update**: Each frame, `PointerQueryState::update()` runs for all tracked elements. It uses the event router's hit test results to determine hover state and element bounds, then computes normalized coordinates, velocity, distance, and angle.

3. **Env resolution**: When a `calc()` expression containing `env(pointer-*)` is evaluated (for opacity, border-radius, rotate, etc.), it resolves against the element's `ElementPointerState`.

4. **Continuous redraw**: While any pointer-tracked element is hovered (or smoothing is active), the system requests redraws to keep values updating.

State is keyed by element string ID (not `LayoutNodeId`), so it persists across tree rebuilds. Smoothed values carry over seamlessly.

---

## Tips

- **Always use `pointer-smoothing`** for visual properties — even a small value like `0.06` eliminates jitter and creates a polished feel.
- **Gate with `pointer-inside`** to prevent effects from firing when the cursor is far away. Multiply: `env(pointer-x) * env(pointer-inside)`.
- **Use `smoothstep` for distance effects** — raw `pointer-distance` drops off linearly, but `smoothstep` creates a natural proximity gradient.
- **Combine freely** — all env variables are independent. Mix position-based rotation with distance-based opacity and hover-gated borders in the same element.
- **Performance**: Only elements with `pointer-space` set are tracked. No per-frame cost for elements that don't opt in.
