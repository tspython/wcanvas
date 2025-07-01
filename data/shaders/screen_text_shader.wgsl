struct ScreenUniforms {
    screen_size : vec2<f32>,
    _pad : vec2<f32>,
};

@group(0) @binding(0) var<uniform> screen : ScreenUniforms;
@group(1) @binding(0) var tex  : texture_2d<f32>;
@group(1) @binding(1) var samp : sampler;

struct VSIn {
    @location(0) pos : vec2<f32>,
    @location(1) uv  : vec2<f32>,
    @location(2) col : vec4<f32>,
};

struct VSOut {
    @builtin(position) clip_position : vec4<f32>,
    @location(0) uv  : vec2<f32>,
    @location(1) col : vec4<f32>,
};

@vertex
fn vs_main(v:VSIn) -> VSOut {
    var o:VSOut;
    let ndc_x = (v.pos.x / screen.screen_size.x) * 2.0 - 1.0;
    let ndc_y = -((v.pos.y / screen.screen_size.y) * 2.0 - 1.0);
    o.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    o.uv = v.uv;
    o.col = v.col;
    return o;
}

fn median(r: f32, g: f32, b: f32) -> f32 {
    return max(min(r, g), min(max(r, g), b));
}

fn smooth_step(edge0: f32, edge1: f32, x: f32) -> f32 {
    let t = clamp((x - edge0) / (edge1 - edge0), 0.0, 1.0);
    return t * t * (3.0 - 2.0 * t);
}

@fragment
fn fs_main(i:VSOut) -> @location(0) vec4<f32> {
    let msdf_sample = textureSample(tex, samp, i.uv);
    
    let distance = median(msdf_sample.r, msdf_sample.g, msdf_sample.b);
    
    let signed_distance = (distance - 0.5) * 12.0; 
    
    let unit_range = 6.0; 
    let screen_px_range = unit_range * length(fwidth(i.uv)) * 64.0; 
    let screen_px_distance = signed_distance / max(screen_px_range, 0.001);
    
    let smoothness = 0.7; 
    let alpha = smooth_step(-smoothness, smoothness, screen_px_distance);
    
    return vec4<f32>(i.col.rgb, i.col.a * alpha);
} 