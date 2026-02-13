//! Image texture management for GPU rendering
//!
//! Manages GPU textures for images and provides rendering support.

use std::sync::Arc;
use wgpu::util::DeviceExt;

/// A GPU image texture ready for rendering
pub struct GpuImage {
    /// The GPU texture
    texture: wgpu::Texture,
    /// Texture view for sampling
    view: wgpu::TextureView,
    /// Image width
    width: u32,
    /// Image height
    height: u32,
}

impl GpuImage {
    /// Create a GPU image from RGBA pixel data
    pub fn from_rgba(
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        pixels: &[u8],
        width: u32,
        height: u32,
        label: Option<&str>,
    ) -> Self {
        let texture = device.create_texture_with_data(
            queue,
            &wgpu::TextureDescriptor {
                label,
                size: wgpu::Extent3d {
                    width,
                    height,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: wgpu::TextureFormat::Rgba8Unorm,
                usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                view_formats: &[],
            },
            wgpu::util::TextureDataOrder::LayerMajor,
            pixels,
        );

        let view = texture.create_view(&wgpu::TextureViewDescriptor::default());

        Self {
            texture,
            view,
            width,
            height,
        }
    }

    /// Get the texture view for binding
    pub fn view(&self) -> &wgpu::TextureView {
        &self.view
    }

    /// Get image dimensions
    pub fn dimensions(&self) -> (u32, u32) {
        (self.width, self.height)
    }

    /// Get image width
    pub fn width(&self) -> u32 {
        self.width
    }

    /// Get image height
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the underlying texture
    pub fn texture(&self) -> &wgpu::Texture {
        &self.texture
    }
}

/// GPU image instance data for batched rendering
///
/// Memory layout (matches shader ImageInstance):
/// - `dst_rect`: `vec4<f32>` (16 bytes) - destination rectangle
/// - `src_uv`: `vec4<f32>` (16 bytes) - source UV coordinates
/// - `tint`: `vec4<f32>` (16 bytes) - tint color
/// - `params`: `vec4<f32>` (16 bytes) - border_radius, opacity, sin_rot, cos_rot
/// - `clip_bounds`: `vec4<f32>` (16 bytes) - clip region
/// - `clip_radius`: `vec4<f32>` (16 bytes) - clip corner radii
/// - `filter_a`: `vec4<f32>` (16 bytes) - grayscale, invert, sepia, hue_rotate_rad
/// - `filter_b`: `vec4<f32>` (16 bytes) - brightness, contrast, saturate, unused
/// - `transform`: `vec4<f32>` (16 bytes) - 2x2 affine matrix [a, b, c, d]
///   Total: 144 bytes
#[repr(C)]
#[derive(Debug, Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct GpuImageInstance {
    /// Destination rectangle (x, y, width, height) in screen pixels
    pub dst_rect: [f32; 4],
    /// Source UV rectangle (u_min, v_min, u_max, v_max) normalized 0-1
    pub src_uv: [f32; 4],
    /// Tint color (RGBA)
    pub tint: [f32; 4],
    /// Parameters: (border_radius, opacity, sin_rot, cos_rot)
    pub params: [f32; 4],
    /// Clip bounds (x, y, width, height) - set to large negative x for no clip
    pub clip_bounds: [f32; 4],
    /// Clip corner radii (top-left, top-right, bottom-right, bottom-left)
    pub clip_radius: [f32; 4],
    /// CSS filter A (grayscale, invert, sepia, hue_rotate_rad)
    pub filter_a: [f32; 4],
    /// CSS filter B (brightness, contrast, saturate, unused)
    pub filter_b: [f32; 4],
    /// 2x2 CSS affine transform [a, b, c, d] applied around quad center.
    /// Identity = [1, 0, 0, 1]. Supports rotation, scale, and skew.
    pub transform: [f32; 4],
}

impl Default for GpuImageInstance {
    fn default() -> Self {
        Self {
            dst_rect: [0.0, 0.0, 100.0, 100.0],
            src_uv: [0.0, 0.0, 1.0, 1.0],
            tint: [1.0, 1.0, 1.0, 1.0],
            params: [0.0, 1.0, 0.0, 1.0], // border_radius=0, opacity=1, sin_rot=0, cos_rot=1
            // Default: no clip (large negative value disables clipping)
            clip_bounds: [-10000.0, -10000.0, 100000.0, 100000.0],
            clip_radius: [0.0; 4],
            // Default filter: identity (no effect)
            filter_a: [0.0, 0.0, 0.0, 0.0], // grayscale=0, invert=0, sepia=0, hue_rotate=0
            filter_b: [1.0, 1.0, 1.0, 0.0], // brightness=1, contrast=1, saturate=1, unused=0
            // Default transform: identity (no rotation, scale, or skew)
            transform: [1.0, 0.0, 0.0, 1.0], // [a, b, c, d] = identity
        }
    }
}

impl GpuImageInstance {
    /// Create a new image instance with no transformations
    pub fn new(x: f32, y: f32, width: f32, height: f32) -> Self {
        Self {
            dst_rect: [x, y, width, height],
            ..Default::default()
        }
    }

    /// Set the source UV coordinates for cropping
    pub fn with_src_uv(mut self, u_min: f32, v_min: f32, u_max: f32, v_max: f32) -> Self {
        self.src_uv = [u_min, v_min, u_max, v_max];
        self
    }

    /// Set a tint color
    pub fn with_tint(mut self, r: f32, g: f32, b: f32, a: f32) -> Self {
        self.tint = [r, g, b, a];
        self
    }

    /// Set border radius for rounded corners
    pub fn with_border_radius(mut self, radius: f32) -> Self {
        self.params[0] = radius;
        self
    }

    /// Set opacity
    pub fn with_opacity(mut self, opacity: f32) -> Self {
        self.params[1] = opacity;
        self
    }

    /// Set rotation via sin/cos values (rotates around quad center)
    /// NOTE: Prefer `with_transform` for full affine (rotation + scale + skew).
    /// This sets params[2..3] which are unused when `transform` is non-identity.
    pub fn with_rotation_sincos(mut self, sin_rot: f32, cos_rot: f32) -> Self {
        self.params[2] = sin_rot;
        self.params[3] = cos_rot;
        self
    }

    /// Set full 2x2 affine transform [a, b, c, d] applied around quad center.
    /// Supports rotation, scale, and skew. Identity = [1, 0, 0, 1].
    pub fn with_transform(mut self, a: f32, b: f32, c: f32, d: f32) -> Self {
        self.transform = [a, b, c, d];
        self
    }

    /// Set rectangular clip region
    pub fn with_clip_rect(mut self, x: f32, y: f32, width: f32, height: f32) -> Self {
        self.clip_bounds = [x, y, width, height];
        self.clip_radius = [0.0; 4];
        self
    }

    /// Set rounded rectangular clip region with uniform radius
    pub fn with_clip_rounded_rect(
        mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        radius: f32,
    ) -> Self {
        self.clip_bounds = [x, y, width, height];
        self.clip_radius = [radius; 4];
        self
    }

    /// Set rounded rectangular clip region with per-corner radii
    #[allow(clippy::too_many_arguments)]
    pub fn with_clip_rounded_rect_corners(
        mut self,
        x: f32,
        y: f32,
        width: f32,
        height: f32,
        tl: f32,
        tr: f32,
        br: f32,
        bl: f32,
    ) -> Self {
        self.clip_bounds = [x, y, width, height];
        self.clip_radius = [tl, tr, br, bl];
        self
    }

    /// Clear clip region (no clipping)
    pub fn with_no_clip(mut self) -> Self {
        self.clip_bounds = [-10000.0, -10000.0, 100000.0, 100000.0];
        self.clip_radius = [0.0; 4];
        self
    }

    /// Set CSS filter parameters
    /// filter_a = (grayscale, invert, sepia, hue_rotate_rad)
    /// filter_b = (brightness, contrast, saturate, 0)
    pub fn with_filter(mut self, filter_a: [f32; 4], filter_b: [f32; 4]) -> Self {
        self.filter_a = filter_a;
        self.filter_b = filter_b;
        self
    }

    /// Get border radius
    pub fn border_radius(&self) -> f32 {
        self.params[0]
    }

    /// Get opacity
    pub fn opacity(&self) -> f32 {
        self.params[1]
    }
}

/// Image rendering context
pub struct ImageRenderingContext {
    /// Device reference
    device: Arc<wgpu::Device>,
    /// Queue reference
    queue: Arc<wgpu::Queue>,
    /// Image sampler (linear filtering)
    sampler_linear: wgpu::Sampler,
    /// Image sampler (nearest filtering, for pixel art)
    sampler_nearest: wgpu::Sampler,
}

impl ImageRenderingContext {
    /// Create a new image rendering context
    pub fn new(device: Arc<wgpu::Device>, queue: Arc<wgpu::Queue>) -> Self {
        let sampler_linear = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Sampler (Linear)"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });

        let sampler_nearest = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("Image Sampler (Nearest)"),
            address_mode_u: wgpu::AddressMode::ClampToEdge,
            address_mode_v: wgpu::AddressMode::ClampToEdge,
            address_mode_w: wgpu::AddressMode::ClampToEdge,
            mag_filter: wgpu::FilterMode::Nearest,
            min_filter: wgpu::FilterMode::Nearest,
            mipmap_filter: wgpu::FilterMode::Nearest,
            ..Default::default()
        });

        Self {
            device,
            queue,
            sampler_linear,
            sampler_nearest,
        }
    }

    /// Create a GPU image from RGBA data
    pub fn create_image(&self, pixels: &[u8], width: u32, height: u32) -> GpuImage {
        GpuImage::from_rgba(&self.device, &self.queue, pixels, width, height, None)
    }

    /// Create a GPU image with a label
    pub fn create_image_labeled(
        &self,
        pixels: &[u8],
        width: u32,
        height: u32,
        label: &str,
    ) -> GpuImage {
        GpuImage::from_rgba(
            &self.device,
            &self.queue,
            pixels,
            width,
            height,
            Some(label),
        )
    }

    /// Get the linear sampler
    pub fn sampler_linear(&self) -> &wgpu::Sampler {
        &self.sampler_linear
    }

    /// Get the nearest sampler
    pub fn sampler_nearest(&self) -> &wgpu::Sampler {
        &self.sampler_nearest
    }

    /// Get the device
    pub fn device(&self) -> &wgpu::Device {
        &self.device
    }

    /// Get the queue
    pub fn queue(&self) -> &wgpu::Queue {
        &self.queue
    }
}
