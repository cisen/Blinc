# Changelog

All notable changes to `blinc_animation` will be documented in this file.

## [0.1.13] - 2026-02-18

### Added

- CSS transitions with automatic detection via `detect_and_start_transitions()` and snapshot/interpolation
- CSS filter animations: blur, drop-shadow, grayscale, sepia, invert, brightness, contrast, saturate, hue-rotate
- Backdrop-filter animation: blur, saturate, brightness
- Text-shadow property animation
- Gradient color stop animation with OBB coordinate fix
- Animated clip-path on hover with keyframe interpolation
- Advanced CSS selectors: `:not()`, `:is()`, `:where()`, structural pseudo-classes
- Outline property animation (width, color, offset)
- Layout animation (width, height, padding, margin, gap) with taffy style updates
- SVG path morphing via `d` attribute in @keyframes
- Corner-shape (superellipse) and overflow-fade animation support

### Fixed

- Easing curve corrections for ease-in, ease-out, ease-in-out
- Transform-origin jitter on hover reverse transitions
- Animation timing for iteration count and fill mode
- Border morph on rounded clip containers
