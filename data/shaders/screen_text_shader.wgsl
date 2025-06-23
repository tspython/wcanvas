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

@fragment
fn fs_main(i:VSOut) -> @location(0) vec4<f32> {
    let a = textureSample(tex, samp, i.uv).r;
    return vec4<f32>(i.col.rgb, i.col.a * a);
} 