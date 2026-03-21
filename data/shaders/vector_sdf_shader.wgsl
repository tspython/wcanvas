// Vector SDF Shape Shader
//
// Renders shapes using Signed Distance Functions for resolution-independent
// vector rendering. Each shape is a simple quad; the fragment shader evaluates
// the SDF per-pixel for perfect edges at any zoom level.

struct Uniforms {
    transform: mat4x4<f32>,
};
@group(0) @binding(0)
var<uniform> uniforms: Uniforms;

struct SdfVertexInput {
    @location(0) position: vec2<f32>,
    @location(1) local_pos: vec2<f32>,
    @location(2) color: vec4<f32>,
    @location(3) shape_params: vec4<f32>,  // [shape_type, half_width, half_height, stroke_width]
    @location(4) fill_params: vec4<f32>,   // [fill_flag, unused, unused, unused]
}

struct SdfVertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) local_pos: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) shape_params: vec4<f32>,
    @location(3) fill_params: vec4<f32>,
}

@vertex
fn vs_main(in: SdfVertexInput) -> SdfVertexOutput {
    var out: SdfVertexOutput;
    out.clip_position = uniforms.transform * vec4<f32>(in.position, 0.0, 1.0);
    out.local_pos = in.local_pos;
    out.color = in.color;
    out.shape_params = in.shape_params;
    out.fill_params = in.fill_params;
    return out;
}

// SDF for a rectangle centered at origin with given half-size
fn sdf_rect(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let d = abs(p) - half_size;
    return length(max(d, vec2<f32>(0.0, 0.0))) + min(max(d.x, d.y), 0.0);
}

// SDF for a circle/ellipse centered at origin
fn sdf_ellipse(p: vec2<f32>, radii: vec2<f32>) -> f32 {
    // Normalize to unit circle space
    let normalized = p / radii;
    return (length(normalized) - 1.0) * min(radii.x, radii.y);
}

// SDF for a diamond (rotated square / rhombus) centered at origin
fn sdf_diamond(p: vec2<f32>, half_size: vec2<f32>) -> f32 {
    let normalized = abs(p) / half_size;
    let d = (normalized.x + normalized.y - 1.0) * min(half_size.x, half_size.y) * 0.7071;
    return d;
}

@fragment
fn fs_main(in: SdfVertexOutput) -> @location(0) vec4<f32> {
    let shape_type = u32(in.shape_params.x + 0.5);
    let half_w = in.shape_params.y;
    let half_h = in.shape_params.z;
    let stroke_width = in.shape_params.w;
    let is_filled = in.fill_params.x > 0.5;

    let p = in.local_pos;

    var d: f32;

    switch shape_type {
        case 0u: {
            // Rectangle
            d = sdf_rect(p, vec2<f32>(half_w, half_h));
        }
        case 1u: {
            // Circle / Ellipse
            d = sdf_ellipse(p, vec2<f32>(half_w, half_h));
        }
        case 2u: {
            // Diamond
            d = sdf_diamond(p, vec2<f32>(half_w, half_h));
        }
        default: {
            d = sdf_ellipse(p, vec2<f32>(half_w, half_h));
        }
    }

    // Anti-aliasing: use screen-space derivatives for smooth edges
    let pixel_dist = length(vec2<f32>(dpdx(d), dpdy(d)));
    let aa_width = max(pixel_dist, 0.5);

    var alpha: f32;
    if is_filled {
        // Filled shape: smooth step at the boundary
        alpha = 1.0 - smoothstep(-aa_width, aa_width, d);
    } else {
        // Stroke only: band around the boundary
        let half_stroke = stroke_width * 0.5;
        let inner_dist = abs(d) - half_stroke;
        alpha = 1.0 - smoothstep(-aa_width, aa_width, inner_dist);
    }

    if alpha < 0.01 {
        discard;
    }

    return vec4<f32>(in.color.rgb, in.color.a * alpha);
}
