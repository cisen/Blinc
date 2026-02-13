// Image rendering shader
// Supports: texture sampling, UV cropping, tinting, rounded corners, opacity, clipping, CSS filters

struct Uniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

struct ImageInstance {
    // Destination rectangle (x, y, width, height) in screen pixels
    @location(0) dst_rect: vec4<f32>,
    // Source UV rectangle (u_min, v_min, u_max, v_max)
    @location(1) src_uv: vec4<f32>,
    // Tint color (RGBA)
    @location(2) tint: vec4<f32>,
    // Border radius, opacity, (unused), (unused)
    @location(3) params: vec4<f32>, // (border_radius, opacity, _, _)
    // Clip bounds (x, y, width, height) - set to large values for no clip
    @location(4) clip_bounds: vec4<f32>,
    // Clip corner radii (top-left, top-right, bottom-right, bottom-left)
    @location(5) clip_radius: vec4<f32>,
    // CSS filter A (grayscale, invert, sepia, hue_rotate_rad)
    @location(6) filter_a: vec4<f32>,
    // CSS filter B (brightness, contrast, saturate, unused)
    @location(7) filter_b: vec4<f32>,
    // 2x2 affine transform (a, b, c, d) applied around quad center
    // Identity = (1, 0, 0, 1). Supports rotation, scale, and skew.
    @location(8) transform: vec4<f32>,
    // Secondary clip bounds (x, y, width, height) - sharp rect for scroll boundary
    @location(9) clip2_bounds: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) tint: vec4<f32>,
    @location(2) local_pos: vec2<f32>,
    @location(3) rect_size: vec2<f32>,
    @location(4) border_radius: f32,
    @location(5) opacity: f32,
    @location(6) world_pos: vec2<f32>,
    @location(7) clip_bounds: vec4<f32>,
    @location(8) clip_radius: vec4<f32>,
    @location(9) filter_a: vec4<f32>,
    @location(10) filter_b: vec4<f32>,
    @location(11) clip2_bounds: vec4<f32>,
}

@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

@group(0) @binding(1)
var image_texture: texture_2d<f32>;

@group(0) @binding(2)
var image_sampler: sampler;

// Vertex indices for a quad (two triangles)
var<private> QUAD_INDICES: array<u32, 6> = array<u32, 6>(0u, 1u, 2u, 2u, 3u, 0u);
var<private> QUAD_POSITIONS: array<vec2<f32>, 4> = array<vec2<f32>, 4>(
    vec2<f32>(0.0, 0.0), // Top-left
    vec2<f32>(1.0, 0.0), // Top-right
    vec2<f32>(1.0, 1.0), // Bottom-right
    vec2<f32>(0.0, 1.0), // Bottom-left
);

@vertex
fn vs_main(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
    instance: ImageInstance,
) -> VertexOutput {
    let quad_index = QUAD_INDICES[vertex_index];
    let local_pos = QUAD_POSITIONS[quad_index];

    // Calculate screen position
    var x = instance.dst_rect.x + local_pos.x * instance.dst_rect.z;
    var y = instance.dst_rect.y + local_pos.y * instance.dst_rect.w;

    // Apply 2x2 affine transform around quad center if non-identity
    // transform = (a, b, c, d) where identity = (1, 0, 0, 1)
    let ta = instance.transform.x;
    let tb = instance.transform.y;
    let tc = instance.transform.z;
    let td = instance.transform.w;
    let has_transform = abs(ta - 1.0) > 0.0001 || abs(tb) > 0.0001 || abs(tc) > 0.0001 || abs(td - 1.0) > 0.0001;
    if has_transform {
        let cx = instance.dst_rect.x + instance.dst_rect.z * 0.5;
        let cy = instance.dst_rect.y + instance.dst_rect.w * 0.5;
        let dx = x - cx;
        let dy = y - cy;
        x = cx + ta * dx + tc * dy;
        y = cy + tb * dx + td * dy;
    }

    // Convert to NDC
    let ndc_x = (x / uniforms.screen_size.x) * 2.0 - 1.0;
    let ndc_y = 1.0 - (y / uniforms.screen_size.y) * 2.0;

    // Interpolate UV coordinates
    let uv = vec2<f32>(
        mix(instance.src_uv.x, instance.src_uv.z, local_pos.x),
        mix(instance.src_uv.y, instance.src_uv.w, local_pos.y),
    );

    var output: VertexOutput;
    output.position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    output.uv = uv;
    output.tint = instance.tint;
    output.local_pos = local_pos * vec2<f32>(instance.dst_rect.z, instance.dst_rect.w);
    output.rect_size = vec2<f32>(instance.dst_rect.z, instance.dst_rect.w);
    output.border_radius = instance.params.x;
    output.opacity = instance.params.y;
    output.world_pos = vec2<f32>(x, y);
    output.clip_bounds = instance.clip_bounds;
    output.clip_radius = instance.clip_radius;
    output.filter_a = instance.filter_a;
    output.filter_b = instance.filter_b;
    output.clip2_bounds = instance.clip2_bounds;

    return output;
}

// SDF for rounded rectangle (uniform radius)
fn rounded_rect_sdf(pos: vec2<f32>, size: vec2<f32>, radius: f32) -> f32 {
    let half_size = size * 0.5;
    let center_pos = pos - half_size;
    let r = min(radius, min(half_size.x, half_size.y));
    let q = abs(center_pos) - half_size + r;
    return min(max(q.x, q.y), 0.0) + length(max(q, vec2<f32>(0.0))) - r;
}

// SDF for rounded rectangle with per-corner radii
fn rounded_rect_sdf_corners(p: vec2<f32>, origin: vec2<f32>, size: vec2<f32>, radius: vec4<f32>) -> f32 {
    let half_size = size * 0.5;
    let center = origin + half_size;
    let rel = p - center;
    let q = abs(rel) - half_size;

    // Select corner radius based on quadrant
    // radius: (top-left, top-right, bottom-right, bottom-left)
    var r: f32;
    if rel.y < 0.0 {
        if rel.x > 0.0 {
            r = radius.y; // top-right
        } else {
            r = radius.x; // top-left
        }
    } else {
        if rel.x > 0.0 {
            r = radius.z; // bottom-right
        } else {
            r = radius.w; // bottom-left
        }
    }

    r = min(r, min(half_size.x, half_size.y));
    let q_adjusted = q + vec2<f32>(r);
    return length(max(q_adjusted, vec2<f32>(0.0))) + min(max(q_adjusted.x, q_adjusted.y), 0.0) - r;
}

// Calculate clip alpha (1.0 = inside clip, 0.0 = outside)
fn calculate_clip_alpha(p: vec2<f32>, clip_bounds: vec4<f32>, clip_radius: vec4<f32>) -> f32 {
    // Check if clip is effectively disabled (large bounds)
    if clip_bounds.x < -9000.0 {
        return 1.0;
    }

    let clip_d = rounded_rect_sdf_corners(p, clip_bounds.xy, clip_bounds.zw, clip_radius);

    // Anti-aliased clip edge
    let aa_width = fwidth(clip_d) * 0.5;
    return 1.0 - smoothstep(-aa_width, aa_width, clip_d);
}

/// Apply CSS filter effects to a color.
/// filter_a = (grayscale, invert, sepia, hue_rotate_rad)
/// filter_b = (brightness, contrast, saturate, 0)
/// Values are in sRGB space (texture uses Rgba8Unorm, no hardware sRGB decode).
fn apply_css_filter(color: vec4<f32>, fa: vec4<f32>, fb: vec4<f32>) -> vec4<f32> {
    var rgb = color.rgb;

    // Grayscale: desaturate using luminance weights
    let grayscale = fa.x;
    if grayscale > 0.0 {
        let lum = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        rgb = mix(rgb, vec3<f32>(lum, lum, lum), grayscale);
    }

    // Sepia: apply sepia tone matrix
    let sepia = fa.z;
    if sepia > 0.0 {
        let sepia_r = dot(rgb, vec3<f32>(0.393, 0.769, 0.189));
        let sepia_g = dot(rgb, vec3<f32>(0.349, 0.686, 0.168));
        let sepia_b = dot(rgb, vec3<f32>(0.272, 0.534, 0.131));
        rgb = mix(rgb, vec3<f32>(sepia_r, sepia_g, sepia_b), sepia);
    }

    // Invert
    let invert = fa.y;
    if invert > 0.0 {
        rgb = mix(rgb, vec3<f32>(1.0) - rgb, invert);
    }

    // Hue-rotate: rotate in RGB space using rotation matrix
    let hue_rad = fa.w;
    if abs(hue_rad) > 0.001 {
        let cos_h = cos(hue_rad);
        let sin_h = sin(hue_rad);
        let w = vec3<f32>(0.2126, 0.7152, 0.0722);
        // Rodrigues-style hue rotation matrix
        let r = vec3<f32>(
            cos_h + (1.0 - cos_h) * w.x,
            (1.0 - cos_h) * w.x * w.y - sin_h * w.z,
            (1.0 - cos_h) * w.x * w.z + sin_h * w.y
        );
        let g = vec3<f32>(
            (1.0 - cos_h) * w.x * w.y + sin_h * w.z,
            cos_h + (1.0 - cos_h) * w.y,
            (1.0 - cos_h) * w.y * w.z - sin_h * w.x
        );
        let b = vec3<f32>(
            (1.0 - cos_h) * w.x * w.z - sin_h * w.y,
            (1.0 - cos_h) * w.y * w.z + sin_h * w.x,
            cos_h + (1.0 - cos_h) * w.z
        );
        rgb = vec3<f32>(dot(rgb, r), dot(rgb, g), dot(rgb, b));
    }

    // Brightness
    let brightness = fb.x;
    rgb = rgb * brightness;

    // Contrast
    let contrast = fb.y;
    rgb = (rgb - vec3<f32>(0.5)) * contrast + vec3<f32>(0.5);

    // Saturate
    let saturate = fb.z;
    if abs(saturate - 1.0) > 0.001 {
        let lum = dot(rgb, vec3<f32>(0.2126, 0.7152, 0.0722));
        rgb = mix(vec3<f32>(lum, lum, lum), rgb, saturate);
    }

    return vec4<f32>(clamp(rgb, vec3<f32>(0.0), vec3<f32>(1.0)), color.a);
}

@fragment
fn fs_main(input: VertexOutput) -> @location(0) vec4<f32> {
    // Early clip test - discard if outside either clip region
    let clip_alpha = calculate_clip_alpha(input.world_pos, input.clip_bounds, input.clip_radius);
    if clip_alpha < 0.001 {
        discard;
    }
    // Secondary clip: sharp rect (scroll boundary), no corner radius
    let clip2_alpha = calculate_clip_alpha(input.world_pos, input.clip2_bounds, vec4<f32>(0.0));
    if clip2_alpha < 0.001 {
        discard;
    }

    // Sample the texture
    var color = textureSample(image_texture, image_sampler, input.uv);

    // Apply tint
    color = color * input.tint;

    // Apply CSS filters (grayscale, invert, sepia, hue-rotate, brightness, contrast, saturate)
    let fa = input.filter_a;
    let fb = input.filter_b;
    if fa.x != 0.0 || fa.y != 0.0 || fa.z != 0.0 || abs(fa.w) > 0.001 || fb.x != 1.0 || fb.y != 1.0 || fb.z != 1.0 {
        color = apply_css_filter(color, fa, fb);
    }

    // Apply opacity
    color.a *= input.opacity;

    // Apply rounded corners if radius > 0
    if input.border_radius > 0.0 {
        let sdf = rounded_rect_sdf(input.local_pos, input.rect_size, input.border_radius);
        // Anti-aliased edge (1 pixel smooth)
        let alpha = 1.0 - smoothstep(-1.0, 1.0, sdf);
        color.a *= alpha;
    }

    // Apply both clip alphas
    color.a *= clip_alpha * clip2_alpha;

    // Output premultiplied alpha for correct blending
    // (blend state uses src_factor: One, dst_factor: OneMinusSrcAlpha)
    color = vec4<f32>(color.rgb * color.a, color.a);

    return color;
}
