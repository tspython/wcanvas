struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
}

struct ScreenUniforms {
    screen_size: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> screen: ScreenUniforms;

@vertex
fn vs_main(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let ndc_x = (vertex.position.x / screen.screen_size.x) * 2.0 - 1.0;
    let ndc_y = -((vertex.position.y / screen.screen_size.y) * 2.0 - 1.0); 
    
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vertex.color;
    return out;
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    return in.color;
} 