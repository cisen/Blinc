# Changelog

All notable changes to `blinc_gpu` will be documented in this file.

## [Unreleased]

### Added

#### Flow Codegen

- Underscore variants for all `StepType` identifiers (e.g. `pattern_noise` alongside `pattern-noise`) for `flow!` macro compatibility

### Changed

- `GpuImageInstance` params layout: `params[2..3]` changed from sin_rot/cos_rot to border_width/packed_border_color; rotation now uses the `transform` field
- Image shader (`image.wgsl`) consolidated VertexOutput from 16 locations to 13 by passing combined `params` vec4

### Removed

- `GpuImageInstance::with_rotation_sincos()` — superseded by `with_transform()` for full 2x2 affine support

### Added

#### 3D SDF Raymarching Pipeline

- Per-element 3D shapes via `shape-3d: box | sphere | cylinder | torus | capsule | group`
- `depth` property for 3D extrusion depth
- `perspective` property for camera distance
- `rotate-x`, `rotate-y` for 3D axis rotation
- `translate-z` for Z-axis translation (closer/farther positioning)
- 32-step raymarching with analytical ray-AABB intersection
- Edge anti-aliasing via closest-approach distance tracking
- Blinn-Phong lighting with configurable `ambient`, `specular`, `light-direction`, `light-intensity`

#### 3D Boolean Operations

- `ShapeDesc` struct for per-shape descriptors in group composition (64 bytes, 4 vec4s)
- `MAX_GROUP_SHAPES` constant (16 shapes per group)
- Boolean SDF operations: `union`, `subtract`, `intersect`
- Smooth boolean operations: `smooth-union`, `smooth-subtract`, `smooth-intersect` with configurable blend radius
- `shape-3d: group` for collecting children into compound SDF via storage buffer

#### UV Mapping

- Automatic UV mapping of background (solid/gradient) onto 3D surface hit points
- Box: face-based projection (front/back, top/bottom, left/right)
- Sphere: spherical coordinate mapping
- Cylinder/torus/capsule: cylindrical coordinate mapping

#### GpuPrimitive Extensions

- `perspective[4]` field: `(sin_rx, cos_rx, persp_d, shape_type)`
- `sdf_3d[4]` field: `(depth, ambient, specular_power, translate_z)`
- `light[4]` field: `(dir_x, dir_y, dir_z, intensity)`
- `filter_a[4]` field: `(grayscale, invert, sepia, hue_rotate_rad)` for CSS filters
- `filter_b[4]` field: `(brightness, contrast, saturate, 0)` for CSS filters

#### Image/SVG Rotation Support

- `GpuImageInstance::with_rotation_sincos()` for applying rotation to image quads
- Image vertex shader (`image.wgsl`) rotates quad vertices around center using sin/cos parameters
- Fixed default `GpuImageInstance` params: `cos_rot` now defaults to 1.0 (was 0.0, collapsed all vertices)

#### Corner Radius Clamping

- Clamp `border-radius` to `min(half_width, half_height)` per CSS spec in:
  - SDF border rendering (fragment shader)
  - Glass shader `sd_rounded_rect`
  - Clip shader `sd_rounded_rect_clip`

#### CSS Filters (GPU)

- `apply_css_filter()` WGSL function: grayscale, invert, sepia, hue-rotate, brightness, contrast, saturate
- Identity-skip guard in fragment shader for zero-cost when no filter is active

#### Kawase Blur Pipeline

- Multi-pass Kawase blur shader with 3 modes: CSS filter (RGBA), shadow (alpha-only), passthrough
- Batched blur passes into single GPU command encoder (was per-pass submission)
- Pre-allocated 8 uniform buffers in `blur_uniforms_pool` (eliminates per-frame buffer creation)
- `apply_blur()` and `apply_shadow_blur()` convenience wrappers
- `calculate_effect_expansion()` for automatic layer texture sizing based on effect parameters
- DPI-scaled layer effects in `render_layer_with_motion`
- XLarge texture pooling in `LayerTextureCache` (>512px textures were previously dropped every frame)

#### Paint Context

- `set_3d_shape()` for configuring per-element 3D shape parameters
- `set_3d_translate_z()` for Z-axis offset
- `set_3d_group_raw()` for compound shape composition from raw float arrays
- 3D transient state management with `clear_3d()` reset
- `set_css_filter()` / `clear_css_filter()` for per-element CSS filter state

#### Glass Backdrop Blur

- Golden-angle spiral blur for simple glass shader (72 samples, 6 rings × 12) replacing 5×5 box blur
- CSS-spec-correct sigma: blur radius = standard deviation (was `radius × 0.5`, producing half-strength blur)
- Sampling extent increased to 2.5× sigma with linear ring spacing for proper Gaussian kernel coverage
- Consistent blur algorithm across both liquid glass (GLASS_SHADER) and simple glass (SIMPLE_GLASS_SHADER) pipelines

### Fixed

- Mix blend mode layers were invisible because `has_layer_effects()` didn't check `blend_mode != Normal`, causing the interleaved z-layer path to strip layer commands from the batch
- Z>0 overlay pass re-rendered blend-mode primitives without blend, overwriting correctly composited results
- Simple glass shader pixelation: replaced crude 25-sample box blur with 72-sample golden-angle spiral, eliminating visible grid artifacts
- Glass blur intensity too weak: corrected Gaussian sigma from `radius * 0.5` to `radius` per CSS spec (blur radius = standard deviation)
- `set_css_filter` and `clear_css_filter` now properly override the `DrawContext` trait (previously only defined as inherent methods, causing no-op dispatch via `&mut dyn DrawContext`)
- Clippy warnings across image.rs, particles.rs, path.rs, primitives.rs, renderer.rs, text.rs

## [0.1.1] - Initial Release

- Initial public release with GPU-accelerated 2D rendering pipeline
