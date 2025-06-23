struct Vertex {
    @location(0) position: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) uv: vec2<f32>,  // For positioning within the rectangle
}

struct VertexOutput {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) world_pos: vec2<f32>,
    @location(2) uv: vec2<f32>,
}

struct ScreenUniforms {
    screen_size: vec2<f32>,
    _padding: vec2<f32>,
}

@group(0) @binding(0)
var<uniform> screen: ScreenUniforms;

fn distance_to_rounded_rect(point: vec2<f32>, center: vec2<f32>, half_size: vec2<f32>, corner_radius: f32) -> f32 {
    let d = abs(point - center) - half_size + corner_radius;
    return length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - corner_radius;
}

@vertex
fn vs_main(vertex: Vertex) -> VertexOutput {
    var out: VertexOutput;
    
    let ndc_x = (vertex.position.x / screen.screen_size.x) * 2.0 - 1.0;
    let ndc_y = -((vertex.position.y / screen.screen_size.y) * 2.0 - 1.0); 
    
    out.clip_position = vec4<f32>(ndc_x, ndc_y, 0.0, 1.0);
    out.color = vertex.color;
    out.world_pos = vertex.position;
    out.uv = vertex.uv;
    
    return out;
}

fn create_gradient(base_color: vec4<f32>, uv: vec2<f32>) -> vec4<f32> {
    let gradient_factor = (uv.y + 1.0) * 0.5;
    let lightness_variation = 0.08;
    
    let lighter = base_color.rgb * (1.0 + lightness_variation);
    let darker = base_color.rgb * (1.0 - lightness_variation);
    
    return vec4<f32>(mix(lighter, darker, gradient_factor), base_color.a);
}

@fragment
fn fs_main(in: VertexOutput) -> @location(0) vec4<f32> {
    let corner_radius = 0.15;
    let border_width = 0.05;
    
    let d = abs(in.uv) - vec2<f32>(1.0 - corner_radius, 1.0 - corner_radius);
    let distance_to_edge = length(max(d, vec2<f32>(0.0))) + min(max(d.x, d.y), 0.0) - corner_radius;
    
    let edge_softness = 0.02;
    let alpha = 1.0 - smoothstep(-edge_softness, edge_softness, distance_to_edge);
    
    var base_color = in.color;
    
    let gradient_factor = (in.uv.y + 1.0) * 0.5;
    let gradient_strength = 0.15;
    
    let top_color = base_color.rgb * (1.0 + gradient_strength);
    let bottom_color = base_color.rgb * (1.0 - gradient_strength);
    
    let gradient_color = mix(top_color, bottom_color, gradient_factor);
    var final_color = vec4<f32>(gradient_color, base_color.a);
    
    final_color.a *= alpha;
    
    if (border_width > 0.0) {
        let border_distance = distance_to_edge + border_width;
        let border_alpha = 1.0 - smoothstep(-edge_softness, edge_softness, border_distance);
        let is_border = border_alpha > alpha && alpha > 0.1;
        
        if (is_border) {
            final_color = vec4<f32>(final_color.rgb * 0.7, final_color.a);
        }
    }
    
    return final_color;
} 