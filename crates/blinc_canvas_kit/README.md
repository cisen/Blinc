# blinc_canvas_kit

> **Part of the [Blinc UI Framework](https://project-blinc.github.io/Blinc)**
>
> This crate is a component of Blinc, a GPU-accelerated UI framework for Rust.
> For full documentation and guides, visit the [Blinc documentation](https://project-blinc.github.io/Blinc).

Interactive canvas toolkit for game editors and node graphs — pan, zoom, spatial indexing, multi-select, marquee selection, and snap-to-grid.

## Overview

`blinc_canvas_kit` provides everything needed to build interactive infinite canvas applications:

- **Viewport Management**: Pan (with momentum), zoom (scroll + pinch), coordinate conversion between screen-space and content-space
- **Spatial Indexing**: Uniform-grid spatial hash for O(1) hit testing and fast range queries, scaling to thousands of elements
- **Multi-Select**: Shift+click to add, Cmd/Ctrl+click to toggle, with selection change callbacks
- **Marquee Selection**: Box-select via tool mode or Shift+drag, with live preview and additive mode
- **Snap-to-Grid**: Round content-space positions to configurable grid spacing
- **Background Patterns**: Infinite viewport-aware dot grid, line grid, and crosshatch patterns with zoom-adaptive density
- **Viewport Culling**: Skip rendering off-screen elements with a simple visibility check
- **Hit Regions**: Register interactive bounding boxes during drawing, with click/hover/drag callbacks

## Quick Start

```rust
use blinc_canvas_kit::prelude::*;

let mut kit = CanvasKit::new("editor")
    .with_background(CanvasBackground::dots().with_spacing(25.0))
    .with_snap(25.0);

kit.on_element_click(|evt| {
    if let Some(id) = &evt.region_id {
        println!("Clicked {id}");
    }
});

kit.on_element_drag(|evt| {
    println!("Dragging {} by ({}, {})", evt.region_id, evt.content_delta.x, evt.content_delta.y);
});

kit.on_selection_change(|evt| {
    println!("Selected: {:?}", evt.selected);
});

// Returns a fully-wired Div with canvas + event handlers
kit.element(|ctx, _bounds| {
    let rect = Rect::new(100.0, 100.0, 200.0, 150.0);
    ctx.fill_rect(rect, 8.0.into(), Brush::Solid(Color::BLUE));
    kit.hit_rect("my_node", rect);
})
```

## Features

### Tool Modes

```rust
// Pan mode (default): background drag pans, Shift+drag for marquee
kit.set_tool(CanvasTool::Pan);

// Select mode: background drag draws marquee
kit.set_tool(CanvasTool::Select);
```

### Selection

```rust
// Query selection state
let sel = kit.selection();
for id in &sel.selected {
    println!("Selected: {id}");
}

// Check individual items (useful in render callbacks for visual feedback)
if kit.is_selected("node_1") {
    // Draw highlight
}

// Programmatic selection
kit.set_selection(HashSet::from(["a".into(), "b".into()]));
kit.clear_selection();
```

### Snap-to-Grid

```rust
// Enable snapping
let kit = CanvasKit::new("editor").with_snap(25.0);

// Use in drag callbacks for precise positioning
kit.on_element_drag(|evt| {
    let target = Point::new(base_x + evt.content_delta.x, base_y + evt.content_delta.y);
    let snapped = kit.snap_point(target);
    // Apply snapped position to your element
});
```

### Viewport Culling

```rust
kit.element(|ctx, _bounds| {
    for node in &all_nodes {
        // Skip rendering off-screen elements
        if kit.is_visible(node.rect) {
            ctx.fill_rect(node.rect, corner_radius, brush);
            kit.hit_rect(&node.id, node.rect);
        }
    }
});
```

### Background Patterns

```rust
// Dot grid (default)
CanvasBackground::dots()

// Line grid
CanvasBackground::grid().with_spacing(40.0)

// Crosshatch (diagonal lines)
CanvasBackground::crosshatch()

// Customized with zoom-adaptive density
CanvasBackground::dots()
    .with_spacing(30.0)
    .with_color(Color::rgba(0.3, 0.3, 0.4, 0.5))
    .with_size(3.0)
    .with_zoom_adaptive(0.3, 5) // Below 30% zoom, show every 5th dot
```

## Architecture

All state persists across UI rebuilds via `BlincContextState` keyed storage. The spatial index is rebuilt each frame during the render callback (via `hit_rect()` calls), while selection, viewport, and interaction state persist.

| Module | Purpose |
|--------|---------|
| `viewport` | Pan/zoom state, coordinate conversion, visibility testing |
| `pan` | Momentum panning with EMA velocity tracking |
| `zoom` | Scroll and pinch zoom handlers |
| `spatial` | Uniform-grid spatial hash for hit testing and range queries |
| `selection` | Multi-select state, marquee drag, tool modes |
| `snap` | Grid snapping for content-space coordinates |
| `background` | Infinite viewport-aware pattern rendering |
| `hit` | Hit region types and event structs |
