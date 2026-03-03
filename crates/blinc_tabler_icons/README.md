# blinc_tabler_icons

> **Part of the [Blinc UI Framework](https://project-blinc.github.io/Blinc)**
>
> This crate is a component of Blinc, a GPU-accelerated UI framework for Rust.
> For full documentation and guides, visit the [Blinc documentation](https://project-blinc.github.io/Blinc).

[Tabler](https://tabler.io/icons) icon library for Blinc UI.

## Overview

`blinc_tabler_icons` provides ~5000 outline and ~1000 filled icons from the Tabler icon set as compile-time constants. Icons are stored as SVG path strings for zero runtime cost.

## Features

- **6000+ Icons**: Complete Tabler icon set (outline + filled)
- **Two Variants**: Stroke-based outline and fill-based filled icons
- **Zero Cost**: Dead code elimination removes unused icons
- **SVG Output**: Generate complete SVG strings
- **Customizable**: Custom stroke width and colors

## Setup

Download the Tabler SVG assets (one-time):

```bash
./scripts/download_tabler_icons.sh
```

## Quick Start

```rust
use blinc_tabler_icons::{outline, filled};
use blinc_layout::prelude::*;

// Outline icon (stroke-based)
svg(&blinc_tabler_icons::to_svg(outline::HOME, 24.0))
    .size(24.0, 24.0)
    .color(Color::WHITE)

// Filled icon
svg(&blinc_tabler_icons::to_svg_filled(filled::HEART, 24.0))
    .size(24.0, 24.0)
    .color(Color::RED)
```

## Available Icons

Icons are organized by variant. Here are some examples:

### Outline (Stroke-based)
```rust
outline::HOME
outline::ARROW_LEFT
outline::ARROW_RIGHT
outline::CHECK
outline::SEARCH
outline::SETTINGS
outline::USER
outline::HEART
outline::STAR
outline::BELL
```

### Filled (Solid)
```rust
filled::HOME
filled::HEART
filled::STAR
filled::BELL
filled::BOOKMARK
filled::CIRCLE_CHECK
```

## Generate SVG String

```rust
use blinc_tabler_icons::{outline, filled, to_svg, to_svg_filled};

// Outline: stroke-based (24x24, stroke-width 2)
let svg_string = to_svg(outline::HOME, 24.0);

// Filled: solid shapes
let svg_string = to_svg_filled(filled::HOME, 24.0);

// Custom stroke width (outline only)
let svg_string = to_svg_with_stroke(outline::HOME, 24.0, 1.5);

// Custom color
let svg_string = to_svg_colored(outline::HOME, 24.0, "#00ff00");
let svg_string = to_svg_filled_colored(filled::HOME, 24.0, "#ff0000");
```

## Icon Constants

All icons are `&'static str` constants containing SVG inner elements:

```rust
// Outline icon (stroke paths)
pub const HOME: &str = r#"<path d="M5 12l-2 0l9 -9l9 9l-2 0"/><path d="..."/>"#;

// Filled icon (fill paths)
pub const HOME: &str = r#"<path d="M12.707 2.293l9 9c.63 .63 ..."/>"#;
```

## Full Icon List

See the [Tabler Icons](https://tabler.io/icons) website for the complete list of available icons. All icon names are converted to SCREAMING_SNAKE_CASE:

- `arrow-right` -> `ARROW_RIGHT`
- `brand-github` -> `BRAND_GITHUB`
- `circle-check` -> `CIRCLE_CHECK`

## License

MIT OR Apache-2.0

Icons are from [Tabler Icons](https://tabler.io/icons) under MIT License.
