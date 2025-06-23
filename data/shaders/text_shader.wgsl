struct CanvasUniforms {
    transform: mat4x4<f32>,
}

@group(0) @binding(0) var<uniform> canvas: CanvasUniforms;
@group(1) @binding(0) var tex: texture_2d<f32>;
@group(1) @binding(1) var samp: sampler;

struct VSIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv:  vec2<f32>,
    @location(2) col: vec4<f32>,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
};

@vertex
fn vs_main(v: VSIn) -> VSOut {
    var o: VSOut;

    let transformed_pos = canvas.transform * vec4<f32>(v.pos, 0.0, 1.0);
    o.pos = transformed_pos;
    o.uv  = v.uv;
    o.col = v.col;
    return o;
}

@fragment
fn fs_main(inp: VSOut) -> @location(0) vec4<f32> {
    let a = textureSample(tex, samp, inp.uv).r;
    return vec4<f32>(inp.col.rgb, inp.col.a * a);
}
