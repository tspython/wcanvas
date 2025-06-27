use crate::{drawing::Tool, vertex::UiVertex};

const PALETTE_COLORS: [[f32; 4]; 6] = [
    [0.0, 0.0, 0.0, 1.0],     
    [0.15, 0.15, 0.15, 1.0],  
    [0.83, 0.0, 0.25, 1.0],   
    [0.0, 0.45, 0.95, 1.0],   
    [1.0, 0.8, 0.0, 1.0],     
    [0.14, 0.75, 0.37, 1.0],  
];

pub struct UiRenderer {
    tool_icons: Vec<ToolIcon>,
    color_palette: Vec<ColorSwatch>,
}

struct ColorSwatch {
    color: [f32; 4],
    position: [f32; 2],
    size: [f32; 2],
}

struct ToolIcon {
    tool: Tool,
    position: [f32; 2],
    size: [f32; 2],
    key_binding: &'static str,
}

impl UiRenderer {
    pub fn new() -> Self {
        let tool_icons = vec![
            ToolIcon {
                tool: Tool::Select,
                position: [10.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "1",
            },
            ToolIcon {
                tool: Tool::Pen,
                position: [60.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "2",
            },
            ToolIcon {
                tool: Tool::Rectangle,
                position: [110.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "3",
            },
            ToolIcon {
                tool: Tool::Circle,
                position: [160.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "4",
            },
            ToolIcon {
                tool: Tool::Arrow,
                position: [210.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "5",
            },
            ToolIcon {
                tool: Tool::Text,
                position: [260.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "6",
            },
            ToolIcon {
                tool: Tool::Line,
                position: [310.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "7",
            },
            ToolIcon {
                tool: Tool::Eraser,
                position: [360.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "8",
            },
        ];

        let color_palette = PALETTE_COLORS.iter().enumerate().map(|(i, &color)| {
            let cols = 2;
            let swatch_size = 30.0;
            let padding = 8.0;
            let start_x = 15.0;
            let start_y = 90.0;
            
            let col = (i % cols) as f32;
            let row = (i / cols) as f32;
            
            ColorSwatch {
                color,
                position: [
                    start_x + col as f32 * (swatch_size + padding),
                    start_y + row as f32 * (swatch_size + padding),
                ],
                size: [swatch_size, swatch_size],
            }
        }).collect();

        Self { tool_icons, color_palette }
    }

    fn draw_tool_icon(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        tool: Tool,
        center: [f32; 2],
        size: f32,
        color: [f32; 4],
    ) {
        match tool {
            Tool::Select => {
                let cursor_size = size * 0.8;
                let tip = [center[0] - cursor_size * 0.3, center[1] - cursor_size * 0.3];
                let left_base = [tip[0] + cursor_size * 0.15, tip[1] + cursor_size * 0.7];
                let right_base = [tip[0] + cursor_size * 0.6, tip[1] + cursor_size * 0.4];
                let notch = [tip[0] + cursor_size * 0.35, tip[1] + cursor_size * 0.55];
                
                self.create_simple_triangle(vertices, indices, index_offset, 
                    tip, left_base, notch, color);
                self.create_simple_triangle(vertices, indices, index_offset, 
                    tip, notch, right_base, color);
            },
            Tool::Pen => {
                let line_size = size * 0.7;
                let thickness = size * 0.135;
                
                let start = [center[0] - line_size * 0.3, center[1] - line_size * 0.3];
                let end = [center[0] + line_size * 0.3, center[1] + line_size * 0.3];
                
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let len = (dx * dx + dy * dy).sqrt();
                let perp_x = -dy / len * thickness * 0.5;
                let perp_y = dx / len * thickness * 0.5;
                
                vertices.extend_from_slice(&[
                    UiVertex { position: [start[0] + perp_x, start[1] + perp_y], color, uv: [0.0, 0.0] },
                    UiVertex { position: [start[0] - perp_x, start[1] - perp_y], color, uv: [0.0, 0.0] },
                    UiVertex { position: [end[0] - perp_x, end[1] - perp_y], color, uv: [0.0, 0.0] },
                    UiVertex { position: [end[0] + perp_x, end[1] + perp_y], color, uv: [0.0, 0.0] },
                ]);
                
                indices.extend_from_slice(&[
                    *index_offset, *index_offset + 1, *index_offset + 2,
                    *index_offset, *index_offset + 2, *index_offset + 3,
                ]);
                *index_offset += 4;
            },
            Tool::Rectangle => {
                let rect_size = size * 0.6;
                let thickness = size * 0.162;
                self.draw_rect_outline(vertices, indices, index_offset,
                    center, [rect_size, rect_size * 0.7], thickness, color);
            },
            Tool::Circle => {
                let radius = size * 0.35;
                let thickness = size * 0.162;
                self.draw_smooth_circle_outline(vertices, indices, index_offset,
                    center, radius, thickness, color);
            },
            Tool::Arrow => {
                let arrow_length = size * 0.7;
                let arrow_width = size * 0.25;
                self.draw_clean_arrow(vertices, indices, index_offset,
                    center, arrow_length, arrow_width, color);
            },
            Tool::Text => {
                let t_size = size * 0.7;
                let thickness = size * 0.16;
                
                self.create_simple_rect(vertices, indices, index_offset,
                    [center[0], center[1] - t_size * 0.35], [t_size * 0.8, thickness], color);
                
                self.create_simple_rect(vertices, indices, index_offset,
                    [center[0], center[1] + thickness * 0.5], [thickness, t_size * 0.8], color);
            },
            Tool::Eraser => {
                let eraser_width = size * 0.4;
                let eraser_height = size * 0.6;
                
                self.create_simple_rect(vertices, indices, index_offset,
                    center, [eraser_width, eraser_height], color);
                
                let band_center = [center[0], center[1] - eraser_height * 0.15];
                self.create_simple_rect(vertices, indices, index_offset,
                    band_center, [eraser_width * 1.1, eraser_height * 0.15], color);
            },
            Tool::Line => {
                let line_size = size * 0.7;
                let thickness = size * 0.135;
                
                let start = [center[0] - line_size * 0.4, center[1] - line_size * 0.2];
                let end = [center[0] + line_size * 0.4, center[1] + line_size * 0.2];
                
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let len = (dx * dx + dy * dy).sqrt();
                let perp_x = -dy / len * thickness * 0.5;
                let perp_y = dx / len * thickness * 0.5;
                
                vertices.extend_from_slice(&[
                    UiVertex { position: [start[0] + perp_x, start[1] + perp_y], color, uv: [0.0, 0.0] },
                    UiVertex { position: [start[0] - perp_x, start[1] - perp_y], color, uv: [0.0, 0.0] },
                    UiVertex { position: [end[0] - perp_x, end[1] - perp_y], color, uv: [0.0, 0.0] },
                    UiVertex { position: [end[0] + perp_x, end[1] + perp_y], color, uv: [0.0, 0.0] },
                ]);
                
                indices.extend_from_slice(&[
                    *index_offset, *index_offset + 1, *index_offset + 2,
                    *index_offset, *index_offset + 2, *index_offset + 3,
                ]);
                *index_offset += 4;
            },
        }
    }

    fn create_rounded_rect(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
        _corner_radius: f32,
        _border_width: f32,
    ) {
        let half_width = size[0] * 0.5;
        let half_height = size[1] * 0.5;
        
        let x0 = center[0] - half_width;
        let y0 = center[1] - half_height;
        let x1 = center[0] + half_width;
        let y1 = center[1] + half_height;
        
        vertices.extend_from_slice(&[
            UiVertex { 
                position: [x0, y0], 
                color, 
                uv: [-1.0, -1.0]
            },
            UiVertex { 
                position: [x1, y0], 
                color, 
                uv: [1.0, -1.0]
            },
            UiVertex { 
                position: [x1, y1], 
                color, 
                uv: [1.0, 1.0]
            },
            UiVertex { 
                position: [x0, y1], 
                color, 
                uv: [-1.0, 1.0]
            },
        ]);
        
        indices.extend_from_slice(&[
            *index_offset, *index_offset + 1, *index_offset + 2,
            *index_offset, *index_offset + 2, *index_offset + 3,
        ]);
        *index_offset += 4;
    }

    fn create_simple_rect(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
    ) {
        let half_width = size[0] * 0.5;
        let half_height = size[1] * 0.5;
        
        vertices.extend_from_slice(&[
            UiVertex { 
                position: [center[0] - half_width, center[1] - half_height], 
                color, 
                uv: [-1.0, -1.0] 
            },
            UiVertex { 
                position: [center[0] + half_width, center[1] - half_height], 
                color, 
                uv: [1.0, -1.0] 
            },
            UiVertex { 
                position: [center[0] + half_width, center[1] + half_height], 
                color, 
                uv: [1.0, 1.0] 
            },
            UiVertex { 
                position: [center[0] - half_width, center[1] + half_height], 
                color, 
                uv: [-1.0, 1.0] 
            },
        ]);
        
        indices.extend_from_slice(&[
            *index_offset, *index_offset + 1, *index_offset + 2,
            *index_offset, *index_offset + 2, *index_offset + 3,
        ]);
        *index_offset += 4;
    }

    fn create_simple_triangle(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
    ) {
        vertices.extend_from_slice(&[
            UiVertex { position: p1, color, uv: [0.0, 0.0] },
            UiVertex { position: p2, color, uv: [0.0, 0.0] },
            UiVertex { position: p3, color, uv: [0.0, 0.0] },
        ]);
        
        indices.extend_from_slice(&[
            *index_offset, *index_offset + 1, *index_offset + 2,
        ]);
        *index_offset += 3;
    }

    fn draw_rect_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: [f32; 2],
        thickness: f32,
        color: [f32; 4],
    ) {
        let half_w = size[0] * 0.5;
        let half_h = size[1] * 0.5;
        let half_t = thickness * 0.5;
        
        self.create_simple_rect(vertices, indices, index_offset,
            [center[0], center[1] - half_h], [size[0], thickness], color);
        self.create_simple_rect(vertices, indices, index_offset,
            [center[0], center[1] + half_h], [size[0], thickness], color);
        self.create_simple_rect(vertices, indices, index_offset,
            [center[0] - half_w, center[1]], [thickness, size[1]], color);
        self.create_simple_rect(vertices, indices, index_offset,
            [center[0] + half_w, center[1]], [thickness, size[1]], color);
    }

    fn draw_circle_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        radius: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let segments = 12;
        for i in 0..segments {
            let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            
            let x1 = center[0] + angle1.cos() * radius;
            let y1 = center[1] + angle1.sin() * radius;
            let x2 = center[0] + angle2.cos() * radius;
            let y2 = center[1] + angle2.sin() * radius;
            
            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let segment_center = [(x1 + x2) * 0.5, (y1 + y2) * 0.5];
                self.create_simple_rect(vertices, indices, index_offset,
                    segment_center, [len, thickness], color);
            }
        }
    }

    fn draw_arrow_shape(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        length: f32,
        width: f32,
        color: [f32; 4],
    ) {
        let shaft_width = width * 0.3;
        let head_width = width;
        let head_length = length * 0.4;
        let shaft_length = length - head_length;
        
        self.create_simple_rect(vertices, indices, index_offset,
            [center[0] - shaft_length * 0.25, center[1]], 
            [shaft_length * 0.5, shaft_width], color);
        
        let tip = [center[0] + length * 0.4, center[1]];
        let base1 = [center[0], center[1] - head_width * 0.5];
        let base2 = [center[0], center[1] + head_width * 0.5];
        self.create_simple_triangle(vertices, indices, index_offset, tip, base1, base2, color);
    }

    fn draw_t_shape(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        width: f32,
        height: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        self.create_simple_rect(vertices, indices, index_offset,
            [center[0], center[1] - height * 0.3], [width, thickness], color);
        
        self.create_simple_rect(vertices, indices, index_offset,
            center, [thickness, height], color);
    }

    fn draw_smooth_circle_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        radius: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let segments = 24;
        let inner_radius = radius - thickness * 0.5;
        let outer_radius = radius + thickness * 0.5;
        
        for i in 0..segments {
            let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            
            let inner1 = [center[0] + angle1.cos() * inner_radius, center[1] + angle1.sin() * inner_radius];
            let outer1 = [center[0] + angle1.cos() * outer_radius, center[1] + angle1.sin() * outer_radius];
            let inner2 = [center[0] + angle2.cos() * inner_radius, center[1] + angle2.sin() * inner_radius];
            let outer2 = [center[0] + angle2.cos() * outer_radius, center[1] + angle2.sin() * outer_radius];
            
            vertices.extend_from_slice(&[
                UiVertex { position: inner1, color, uv: [0.0, 0.0] },
                UiVertex { position: outer1, color, uv: [0.0, 0.0] },
                UiVertex { position: outer2, color, uv: [0.0, 0.0] },
                UiVertex { position: inner2, color, uv: [0.0, 0.0] },
            ]);
            
            indices.extend_from_slice(&[
                *index_offset, *index_offset + 1, *index_offset + 2,
                *index_offset, *index_offset + 2, *index_offset + 3,
            ]);
            *index_offset += 4;
        }
    }

    fn draw_clean_arrow(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        length: f32,
        width: f32,
        color: [f32; 4],
    ) {
        let shaft_width = width * 0.4;
        let head_width = width;
        let head_length = length * 0.4;
        let shaft_length = length - head_length;
        
        let shaft_center = [center[0] - head_length * 0.25, center[1]];
        self.create_simple_rect(vertices, indices, index_offset,
            shaft_center, [shaft_length * 0.5, shaft_width], color);
        
        let tip = [center[0] + length * 0.4, center[1]];
        let base1 = [center[0] - head_length * 0.1, center[1] - head_width * 0.5];
        let base2 = [center[0] - head_length * 0.1, center[1] + head_width * 0.5];
        self.create_simple_triangle(vertices, indices, index_offset, tip, base1, base2, color);
    }

    fn draw_text_icon(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: f32,
        color: [f32; 4],
    ) {
        let thickness = size * 0.18;
        let height = size * 0.9;
        let width = size * 0.8;
        
        let left_angle = 0.3;
        let left_center = [center[0] - width * 0.25, center[1] + height * 0.1];
        let left_length = height * 0.8;
        
        let cos_a = (left_angle as f32).cos();
        let sin_a = (left_angle as f32).sin();
        let half_thick = thickness * 0.5;
        let half_len = left_length * 0.5;
        
        let left_corners = [
            [left_center[0] - cos_a * half_len + sin_a * half_thick, left_center[1] - sin_a * half_len - cos_a * half_thick],
            [left_center[0] + cos_a * half_len + sin_a * half_thick, left_center[1] + sin_a * half_len - cos_a * half_thick],
            [left_center[0] + cos_a * half_len - sin_a * half_thick, left_center[1] + sin_a * half_len + cos_a * half_thick],
            [left_center[0] - cos_a * half_len - sin_a * half_thick, left_center[1] - sin_a * half_len + cos_a * half_thick],
        ];
        
        vertices.extend_from_slice(&[
            UiVertex { position: left_corners[0], color, uv: [0.0, 0.0] },
            UiVertex { position: left_corners[1], color, uv: [0.0, 0.0] },
            UiVertex { position: left_corners[2], color, uv: [0.0, 0.0] },
            UiVertex { position: left_corners[3], color, uv: [0.0, 0.0] },
        ]);
        
        indices.extend_from_slice(&[
            *index_offset, *index_offset + 1, *index_offset + 2,
            *index_offset, *index_offset + 2, *index_offset + 3,
        ]);
        *index_offset += 4;
        
        let right_center = [center[0] + width * 0.25, center[1] + height * 0.1];
        let right_corners = [
            [right_center[0] + cos_a * half_len + sin_a * half_thick, right_center[1] - sin_a * half_len - cos_a * half_thick],
            [right_center[0] - cos_a * half_len + sin_a * half_thick, right_center[1] + sin_a * half_len - cos_a * half_thick],
            [right_center[0] - cos_a * half_len - sin_a * half_thick, right_center[1] + sin_a * half_len + cos_a * half_thick],
            [right_center[0] + cos_a * half_len - sin_a * half_thick, right_center[1] - sin_a * half_len + cos_a * half_thick],
        ];
        
        vertices.extend_from_slice(&[
            UiVertex { position: right_corners[0], color, uv: [0.0, 0.0] },
            UiVertex { position: right_corners[1], color, uv: [0.0, 0.0] },
            UiVertex { position: right_corners[2], color, uv: [0.0, 0.0] },
            UiVertex { position: right_corners[3], color, uv: [0.0, 0.0] },
        ]);
        
        indices.extend_from_slice(&[
            *index_offset, *index_offset + 1, *index_offset + 2,
            *index_offset, *index_offset + 2, *index_offset + 3,
        ]);
        *index_offset += 4;
        
        let crossbar_center = [center[0], center[1] + height * 0.15];
        self.create_simple_rect(vertices, indices, index_offset,
            crossbar_center, [width * 0.5, thickness], color);
    }

    fn generate_color_picker(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        current_color: [f32; 4],
    ) {
        let swatch_size = 30.0;
        let padding = 8.0;
        let start_x = 15.0;
        let start_y = 90.0;

        for (i, color) in PALETTE_COLORS.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;

            let center = [
                start_x + col * (swatch_size + padding) + swatch_size * 0.5,
                start_y + row * (swatch_size + padding) + swatch_size * 0.5,
            ];

            let is_selected = (color[0] - current_color[0]).abs() < 0.01 
                && (color[1] - current_color[1]).abs() < 0.01 
                && (color[2] - current_color[2]).abs() < 0.01;

            let border_width = if is_selected { 2.0 } else { 1.0 };
            let final_color = if is_selected {
                [color[0] * 1.1, color[1] * 1.1, color[2] * 1.1, color[3]]
            } else {
                *color
            };

            self.create_rounded_rect(
                vertices,
                indices,
                index_offset,
                center,
                [swatch_size, swatch_size],
                final_color,
                6.0,
                border_width,
            );
        }
    }

    fn generate_toolbar(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        current_tool: Tool,
        screen_size: (f32, f32),
    ) {
        let icon_size = 36.0;
        let icon_spacing = 12.0;
        let toolbar_padding = 20.0;
        let num_icons = self.tool_icons.len() as f32;
        
        // Add macOS titlebar padding
        #[cfg(target_os = "macos")]
        let titlebar_padding = 1.0; // Minimal padding - toolbar right under titlebar!
        #[cfg(not(target_os = "macos"))]
        let titlebar_padding = 0.0;
        
        let toolbar_width = num_icons * icon_size + (num_icons - 1.0) * icon_spacing + 2.0 * toolbar_padding;
        let toolbar_height = icon_size + 2.0 * toolbar_padding;
        let toolbar_center = [screen_size.0 / 2.0, titlebar_padding + toolbar_padding + toolbar_height / 2.0];
        
        let shadow_offset = 3.0;
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [toolbar_center[0] + shadow_offset, toolbar_center[1] + shadow_offset],
            [toolbar_width, toolbar_height],
            [0.0, 0.0, 0.0, 0.15],
            10.0,
            0.0,
        );

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            toolbar_center,
            [toolbar_width, toolbar_height],
            [0.96, 0.96, 0.97, 0.98],
            10.0,
            1.5,
        );

        let start_x = toolbar_center[0] - toolbar_width / 2.0 + toolbar_padding + icon_size / 2.0;
        
        for (i, icon) in self.tool_icons.iter().enumerate() {
            let x = start_x + i as f32 * (icon_size + icon_spacing);
            let y = toolbar_center[1];
            
            let is_selected = icon.tool == current_tool;
            
            let button_color = if is_selected {
                [0.25, 0.55, 0.95, 1.0]
            } else {
                [0.85, 0.85, 0.87, 1.0]
            };
            
            let luminance = 0.299 * button_color[0] + 0.587 * button_color[1] + 0.114 * button_color[2];
            let text_color = if luminance < 0.5 {
                [1.0, 1.0, 1.0, 1.0] 
            } else {
                [0.2, 0.2, 0.2, 1.0] 
            };
            
            self.create_rounded_rect(
                vertices,
                indices,
                index_offset,
                [x, y],
                [icon_size, icon_size],
                button_color,
                8.0,
                if is_selected { 0.0 } else { 1.5 },
            );
            
            let icon_color = if luminance < 0.5 {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.2, 0.2, 0.2, 1.0]
            };
            
            self.draw_tool_icon(
                vertices,
                indices,
                index_offset,
                icon.tool,
                [x, y],
                icon_size * 0.5,
                icon_color,
            );
        }
    }

    fn generate_zoom_indicator(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        screen_size: (f32, f32),
    ) {
        let zoom_bg_width = 80.0;
        let zoom_bg_height = 25.0;
        let zoom_bg_center = [
            15.0 + zoom_bg_width * 0.5,
            screen_size.1 - 15.0 - zoom_bg_height * 0.5,
        ];
        
        let shadow_offset = 2.0;
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [zoom_bg_center[0] + shadow_offset, zoom_bg_center[1] + shadow_offset],
            [zoom_bg_width, zoom_bg_height],
            [0.0, 0.0, 0.0, 0.1],
            6.0,
            0.0,
        );
        
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            zoom_bg_center,
            [zoom_bg_width, zoom_bg_height],
            [0.2, 0.2, 0.2, 0.9],
            6.0,
            1.0,
        );
    }

    pub fn generate_ui_vertices(&self, current_tool: Tool, current_color: [f32; 4], screen_size: (f32, f32), _zoom_level: f32) -> (Vec<UiVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        self.generate_color_picker(&mut vertices, &mut indices, &mut index_offset, current_color);
        self.generate_toolbar(&mut vertices, &mut indices, &mut index_offset, current_tool, screen_size);
        self.generate_zoom_indicator(&mut vertices, &mut indices, &mut index_offset, screen_size);

        (vertices, indices)
    }

    pub fn handle_color_click(&self, mouse_pos: [f32; 2]) -> Option<[f32; 4]> {
        let swatch_size = 30.0;
        let padding = 8.0;
        let start_x = 15.0;
        let start_y = 90.0;

        for (i, &color) in PALETTE_COLORS.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;
            
            let x = start_x + col * (swatch_size + padding);
            let y = start_y + row * (swatch_size + padding);
            
            if mouse_pos[0] >= x && mouse_pos[0] <= x + swatch_size &&
               mouse_pos[1] >= y && mouse_pos[1] <= y + swatch_size {
                return Some(color);
            }
        }
        None
    }

    pub fn handle_click(&self, mouse_pos: [f32; 2], screen_size: (f32, f32)) -> Option<Tool> {
        let icon_size = 36.0; 
        let icon_spacing = 12.0; 
        let num_icons = self.tool_icons.len() as f32;
        let toolbar_padding = 20.0;
        
        // Add macOS titlebar padding
        #[cfg(target_os = "macos")]
        let titlebar_padding = 1.0;
        #[cfg(not(target_os = "macos"))]
        let titlebar_padding = 0.0;
        
        let toolbar_width = num_icons * icon_size + (num_icons - 1.0) * icon_spacing + 2.0 * toolbar_padding;
        let toolbar_height = icon_size + 2.0 * toolbar_padding;
        let toolbar_center = [screen_size.0 / 2.0, titlebar_padding + toolbar_padding + toolbar_height / 2.0];
        
        let toolbar_left = toolbar_center[0] - toolbar_width / 2.0;
        let toolbar_right = toolbar_center[0] + toolbar_width / 2.0;
        let toolbar_top = toolbar_center[1] - toolbar_height / 2.0;
        let toolbar_bottom = toolbar_center[1] + toolbar_height / 2.0;
        
        if mouse_pos[0] < toolbar_left || mouse_pos[0] > toolbar_right ||
           mouse_pos[1] < toolbar_top || mouse_pos[1] > toolbar_bottom {
            return None;
        }
        
        let start_x = toolbar_center[0] - toolbar_width / 2.0 + toolbar_padding;
        
        for (i, icon) in self.tool_icons.iter().enumerate() {
            let button_left = start_x + i as f32 * (icon_size + icon_spacing);
            let button_right = button_left + icon_size;
            let button_top = toolbar_center[1] - icon_size / 2.0;
            let button_bottom = toolbar_center[1] + icon_size / 2.0;
            
            if mouse_pos[0] >= button_left && mouse_pos[0] <= button_right &&
               mouse_pos[1] >= button_top && mouse_pos[1] <= button_bottom {
                return Some(icon.tool);
            }
        }
        
        None
    }

    pub fn is_mouse_over_ui(&self, mouse_pos: [f32; 2], screen_size: (f32, f32)) -> bool {
        let icon_size = 36.0;
        let icon_spacing = 12.0;
        let toolbar_padding = 20.0;
        let num_icons = self.tool_icons.len() as f32;
        
        // Add macOS titlebar padding
        #[cfg(target_os = "macos")]
        let titlebar_padding = 1.0;
        #[cfg(not(target_os = "macos"))]
        let titlebar_padding = 0.0;
        
        let toolbar_width = num_icons * icon_size + (num_icons - 1.0) * icon_spacing + 2.0 * toolbar_padding;
        let toolbar_height = icon_size + 2.0 * toolbar_padding;
        let toolbar_center = [screen_size.0 / 2.0, titlebar_padding + toolbar_padding + toolbar_height / 2.0];
        
        let toolbar_left = toolbar_center[0] - toolbar_width / 2.0;
        let toolbar_right = toolbar_center[0] + toolbar_width / 2.0;
        let toolbar_top = toolbar_center[1] - toolbar_height / 2.0;
        let toolbar_bottom = toolbar_center[1] + toolbar_height / 2.0;
        
        if mouse_pos[0] >= toolbar_left && mouse_pos[0] <= toolbar_right &&
           mouse_pos[1] >= toolbar_top && mouse_pos[1] <= toolbar_bottom {
            return true;
        }

        let swatch_size = 30.0;
        let padding = 8.0;
        let start_x = 15.0;
        let start_y = 90.0;
        let cols = 2;
        let rows = (PALETTE_COLORS.len() + cols - 1) / cols;
        
        let palette_width = cols as f32 * swatch_size + (cols - 1) as f32 * padding;
        let palette_height = rows as f32 * swatch_size + (rows - 1) as f32 * padding;
        
        mouse_pos[0] >= start_x && mouse_pos[0] <= start_x + palette_width &&
        mouse_pos[1] >= start_y && mouse_pos[1] <= start_y + palette_height
    }

    pub fn generate_toolbar_icons(
        &self,
        text_renderer: &mut crate::text_renderer::TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        current_tool: Tool,
        screen_size: (f32, f32),
    ) {
        let icon_size = 36.0;
        let icon_spacing = 12.0;
        let toolbar_padding = 20.0;
        let num_icons = self.tool_icons.len() as f32;
        
        // Add macOS titlebar padding
        #[cfg(target_os = "macos")]
        let titlebar_padding = 1.0;
        #[cfg(not(target_os = "macos"))]
        let titlebar_padding = 0.0;
        
        let toolbar_width = num_icons * icon_size + (num_icons - 1.0) * icon_spacing + 2.0 * toolbar_padding;
        let toolbar_center = [screen_size.0 / 2.0, titlebar_padding + toolbar_padding + (icon_size + 2.0 * toolbar_padding) / 2.0];
        let start_x = toolbar_center[0] - toolbar_width / 2.0 + toolbar_padding + icon_size / 2.0;
        
        for (i, icon) in self.tool_icons.iter().enumerate() {
            let x = start_x + i as f32 * (icon_size + icon_spacing);
            let y = toolbar_center[1];
            
            let is_selected = icon.tool == current_tool;
            
            let button_color = if is_selected {
                [0.25, 0.55, 0.95, 1.0]
            } else {
                [0.85, 0.85, 0.87, 1.0]
            };
            
            let luminance = 0.299 * button_color[0] + 0.587 * button_color[1] + 0.114 * button_color[2];
            let text_color = if luminance < 0.5 {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.2, 0.2, 0.2, 1.0]
            };
            
            let icon_text = match icon.tool {
                Tool::Select => "↖",
                Tool::Pen => "✎",
                Tool::Rectangle => "▭",
                Tool::Circle => "○",
                Tool::Arrow => "→",
                Tool::Text => "T",
                Tool::Eraser => "⌫",
                Tool::Line => "|",
            };
            
            let text_pos = [x - 8.0, y + 6.0];
            
            text_renderer.add_screen_label(
                device,
                queue,
                icon_text,
                text_pos,
                16.0,
                text_color,
            );
        }
    }
} 