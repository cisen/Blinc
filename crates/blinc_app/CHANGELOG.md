# Changelog

All notable changes to `blinc_app` will be documented in this file.

## [Unreleased]

### Added

#### SVG CSS Animations

- SVG fill, stroke, stroke-width animatable via `@keyframes` and CSS transitions
- Stroke-dasharray/dashoffset animation for SVG line-drawing effects
- SVG path morphing via `d: path("...")` CSS animation (cubic bezier interpolation)
- SVG sub-element metadata extraction (`extract_element_metadata`) for future per-element targeting
- `svg_animation_demo` example demonstrating all SVG animation phases

### Fixed

- Text and SVG elements now clip to scroll container boundaries (regression from dual-clip refactor)
- SVG own-transform applied correctly (not just inherited parent transforms)
- CSS `transform: rotate()` animation uses original angle values instead of lossy atan2 decomposition
- Performance: SVG string manipulation only runs on cache miss (not every frame)

#### 3D SDF & Styling Demo

- Expanded `styling_demo` example with 3D shape showcases (box, sphere, cylinder, torus, capsule)
- 3D boolean operations demo (union, subtract, intersect, smooth variants)
- 3D group composition examples with compound shapes
- UV-mapped gradient backgrounds on 3D surfaces
- `translate-z` depth positioning examples
- Blinn-Phong lighting configuration examples

#### Music Player Glass Card Demo

- `music_player` example: iOS-style "Now Playing" card with liquid glass morphism
- All visual styling driven by CSS via `ctx.add_css()`
- Glass card with `backdrop-filter: liquid-glass()` refracted bevel borders
- Album art, song info, progress bar with track glow animation
- Playback controls with glass icon wrappers
- Hover effects: icon/badge scale + glass brightening + shadow deepening + SVG tint transitions
- Progress bar hover-reveal: height transition with overflow clip, opacity-faded time labels

#### SVG CSS Transform Propagation

- SVGs now inherit CSS transforms from ancestor elements
- Affine transform decomposed into scale (applied to SVG bounds) + rotation (GPU shader)
- Layout recomputed after state style changes that affect layout (visibility, display, height, etc.)

#### Stylesheet Runtime Integration

- CSS animation support wired through the app runtime
- `backdrop-filter` property support in windowed runner
- Stylesheet base styles applied after tree construction
- CSS transition ticking and application in frame loop
- Animated layout property support with per-frame `compute_layout()` recomputation

#### Styling Demo Enhancements

- CSS filter hover demo (`.filter-card:hover` with brightness, saturate, contrast)
- Filter blur & drop-shadow demos (static blur, hover transition, keyframe animation, combo)
- Backdrop-filter animation demos (static blur, hover transition, blur+saturate combo, keyframe pulse)
- `:is()` / `:where()` / `*-of-type` selector demos

#### CSS Form Input Styling

- Form input styling demo section with CSS-styled TextInput and TextArea widgets
- `#demo-input` with `:hover`, `:focus`, `::placeholder` pseudo-class/element demos
- `#accent-input` with warm color scheme (yellow/amber) CSS styling
- `#disabled-input` with `opacity: 0.5` disabled state demo
- `#demo-textarea` with `:hover`, `:focus` CSS styling and `caret-color` demo
- Text input focus bridge to EventRouter for `:focus` CSS matching in windowed runner

### Fixed

- Backdrop-filter demo parent containers now include `rounded(12.0)` so glass corner radius is visible against page background
- CSS animation ticking moved to synchronous main-thread execution to eliminate phase jitter caused by background-thread timing misalignment
- Mid-frame transition redraw: transitions created during `apply_complex_selector_styles` now properly trigger frame requests (prevents stalled hover-leave animations)
- iOS runner cleanup for platform trait consistency
- Clippy warnings in windowed.rs and ios.rs

## [0.1.12] - 2025-01-19

### Added
- Momentum scrolling for touch devices with velocity tracking
- `dispatch_scroll_chain_with_time()` method for mobile scroll dispatch with time-based velocity
- Single-threaded animation scheduler for mobile efficiency

### Changed
- Android render loop now uses vsync for frame pacing instead of manual timing
- Non-blocking poll when animating, 100ms idle timeout for power saving
- Re-enabled scroll physics for bounce animations on Android

### Fixed
- Animation smoothness on Android by removing mutex contention between threads
- Double-waiting issue that was cutting frame rate in half
- Added expected cfg values for fuchsia and ohos targets to fix CI warnings

## [0.1.1] - Initial Release

- Initial public release with desktop, Android, and iOS support
