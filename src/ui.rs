use crate::{drawing::Tool, vertex::Vertex};

pub struct UiRenderer {
    tool_icons: Vec<ToolIcon>,
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
                tool: Tool::Eraser,
                position: [310.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "7",
            },
        ];

        Self { tool_icons }
    }

    pub fn generate_ui_vertices(&self, current_tool: Tool, screen_size: (f32, f32), zoom_level: f32) -> (Vec<Vertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        // Create a floating panel style with shadow effect - centered at top of screen
        let panel_width = 350.0;
        let panel_height = 50.0;
        let panel_left = (screen_size.0 - panel_width) / 2.0;
        let panel_right = panel_left + panel_width;
        let panel_top = 15.0;
        let panel_bottom = panel_top + panel_height;
        let shadow_offset = 2.0;
        
        // Calculate icon positions based on centered panel
        let icon_size = 40.0;
        let num_icons = self.tool_icons.len() as f32;
        let total_icon_width = num_icons * icon_size;
        let spacing = (panel_width - total_icon_width - 20.0) / (num_icons - 1.0); // 20.0 for padding
        let icon_start_x = panel_left + 10.0; // 10px padding from left

        // Shadow (darker background slightly offset)
        let shadow_bg = [
            Vertex {
                position: [panel_left + shadow_offset, panel_top + shadow_offset],
                color: [0.0, 0.0, 0.0, 0.2],
            },
            Vertex {
                position: [panel_right + shadow_offset, panel_top + shadow_offset],
                color: [0.0, 0.0, 0.0, 0.2],
            },
            Vertex {
                position: [panel_right + shadow_offset, panel_bottom + shadow_offset],
                color: [0.0, 0.0, 0.0, 0.2],
            },
            Vertex {
                position: [panel_left + shadow_offset, panel_bottom + shadow_offset],
                color: [0.0, 0.0, 0.0, 0.2],
            },
        ];
        vertices.extend_from_slice(&shadow_bg);
        indices.extend_from_slice(&[
            index_offset, index_offset + 1, index_offset + 2,
            index_offset, index_offset + 2, index_offset + 3,
        ]);
        index_offset += 4;

        let panel_bg = [
            Vertex {
                position: [panel_left, panel_top],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [panel_right, panel_top],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [panel_right, panel_bottom],
                color: [1.0, 1.0, 1.0, 1.0],
            },
            Vertex {
                position: [panel_left, panel_bottom],
                color: [1.0, 1.0, 1.0, 1.0],
            },
        ];
        vertices.extend_from_slice(&panel_bg);
        indices.extend_from_slice(&[
            index_offset, index_offset + 1, index_offset + 2,
            index_offset, index_offset + 2, index_offset + 3,
        ]);
        index_offset += 4;

        let border_color = [0.85, 0.85, 0.85, 1.0];
        let border_width = 1.0;
        
        self.add_line(&mut vertices, &mut indices, &mut index_offset,
            [panel_left, panel_top], [panel_right, panel_top], border_width, border_color);
        self.add_line(&mut vertices, &mut indices, &mut index_offset,
            [panel_left, panel_bottom], [panel_right, panel_bottom], border_width, border_color);
        self.add_line(&mut vertices, &mut indices, &mut index_offset,
            [panel_left, panel_top], [panel_left, panel_bottom], border_width, border_color);
        self.add_line(&mut vertices, &mut indices, &mut index_offset,
            [panel_right, panel_top], [panel_right, panel_bottom], border_width, border_color);

        for (i, icon) in self.tool_icons.iter().enumerate() {
            let is_selected = icon.tool == current_tool;
            
            let icon_x = icon_start_x + i as f32 * (icon_size + spacing);
            let icon_y = panel_top + 5.0; // 5px padding from top
            let icon_pos = [icon_x, icon_y];
            let icon_size_vec = [icon_size, icon_size];
            
            if is_selected {
                let bg_color = [0.2, 0.5, 1.0, 1.0];
                vertices.push(Vertex {
                    position: icon_pos,
                    color: bg_color,
                });
                vertices.push(Vertex {
                    position: [icon_pos[0] + icon_size_vec[0], icon_pos[1]],
                    color: bg_color,
                });
                vertices.push(Vertex {
                    position: [icon_pos[0] + icon_size_vec[0], icon_pos[1] + icon_size_vec[1]],
                    color: bg_color,
                });
                vertices.push(Vertex {
                    position: [icon_pos[0], icon_pos[1] + icon_size_vec[1]],
                    color: bg_color,
                });

                indices.extend_from_slice(&[
                    index_offset, index_offset + 1, index_offset + 2,
                    index_offset, index_offset + 2, index_offset + 3,
                ]);
                index_offset += 4;
            } else {
                let border_color = [0.9, 0.9, 0.9, 0.6];
                let border_width = 1.0;
                
                self.add_line(&mut vertices, &mut indices, &mut index_offset,
                    icon_pos, 
                    [icon_pos[0] + icon_size_vec[0], icon_pos[1]], 
                    border_width, border_color);
                self.add_line(&mut vertices, &mut indices, &mut index_offset,
                    [icon_pos[0], icon_pos[1] + icon_size_vec[1]], 
                    [icon_pos[0] + icon_size_vec[0], icon_pos[1] + icon_size_vec[1]], 
                    border_width, border_color);
                self.add_line(&mut vertices, &mut indices, &mut index_offset,
                    icon_pos, 
                    [icon_pos[0], icon_pos[1] + icon_size_vec[1]], 
                    border_width, border_color);
                self.add_line(&mut vertices, &mut indices, &mut index_offset,
                    [icon_pos[0] + icon_size_vec[0], icon_pos[1]], 
                    [icon_pos[0] + icon_size_vec[0], icon_pos[1] + icon_size_vec[1]], 
                    border_width, border_color);
            }

            let icon_color = if is_selected {
                [0.8, 0.3, 1.0, 1.0]
            } else {
                [0.3, 0.3, 0.3, 1.0] 
            };
            let center = [
                icon_pos[0] + icon_size_vec[0] / 2.0,
                icon_pos[1] + icon_size_vec[1] / 2.0,
            ];

            match icon.tool {
                Tool::Select => {
                    let cursor_verts = [
                        Vertex {
                            position: [center[0] - 5.0, center[1] - 8.0],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0] + 5.0, center[1]],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0], center[1] + 8.0],
                            color: icon_color,
                        },
                    ];
                    vertices.extend_from_slice(&cursor_verts);
                    indices.extend_from_slice(&[index_offset, index_offset + 1, index_offset + 2]);
                    index_offset += 3;
                }
                Tool::Pen => {
                    self.add_line(
                        &mut vertices,
                        &mut indices,
                        &mut index_offset,
                        [center[0] - 8.0, center[1] - 8.0],
                        [center[0] + 8.0, center[1] + 8.0],
                        2.0,
                        icon_color,
                    );
                }
                Tool::Rectangle => {
                    let _rect_verts = [
                        Vertex {
                            position: [center[0] - 8.0, center[1] - 6.0],
                            color: [0.0, 0.0, 0.0, 0.0], 
                        },
                        Vertex {
                            position: [center[0] + 8.0, center[1] - 6.0],
                            color: [0.0, 0.0, 0.0, 0.0],
                        },
                        Vertex {
                            position: [center[0] + 8.0, center[1] + 6.0],
                            color: [0.0, 0.0, 0.0, 0.0],
                        },
                        Vertex {
                            position: [center[0] - 8.0, center[1] + 6.0],
                            color: [0.0, 0.0, 0.0, 0.0],
                        },
                    ];
                    
                    self.add_line(&mut vertices, &mut indices, &mut index_offset,
                        [center[0] - 8.0, center[1] - 6.0], [center[0] + 8.0, center[1] - 6.0], 2.0, icon_color);
                    self.add_line(&mut vertices, &mut indices, &mut index_offset,
                        [center[0] + 8.0, center[1] - 6.0], [center[0] + 8.0, center[1] + 6.0], 2.0, icon_color);
                    self.add_line(&mut vertices, &mut indices, &mut index_offset,
                        [center[0] + 8.0, center[1] + 6.0], [center[0] - 8.0, center[1] + 6.0], 2.0, icon_color);
                    self.add_line(&mut vertices, &mut indices, &mut index_offset,
                        [center[0] - 8.0, center[1] + 6.0], [center[0] - 8.0, center[1] - 6.0], 2.0, icon_color);
                }
                Tool::Circle => {
                    const SEGMENTS: u32 = 16;
                    for i in 0..SEGMENTS {
                        let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                        let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                        
                        let p1 = [
                            center[0] + angle1.cos() * 8.0,
                            center[1] + angle1.sin() * 8.0,
                        ];
                        let p2 = [
                            center[0] + angle2.cos() * 8.0,
                            center[1] + angle2.sin() * 8.0,
                        ];
                        
                        self.add_line(&mut vertices, &mut indices, &mut index_offset, p1, p2, 2.0, icon_color);
                    }
                }
                Tool::Arrow => {
                    self.add_line(
                        &mut vertices,
                        &mut indices,
                        &mut index_offset,
                        [center[0] - 8.0, center[1] + 5.0],
                        [center[0] + 5.0, center[1] - 5.0],
                        2.0,
                        icon_color,
                    );
                    vertices.extend_from_slice(&[
                        Vertex {
                            position: [center[0] + 5.0, center[1] - 5.0],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0] + 1.0, center[1] - 5.0],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0] + 5.0, center[1] - 1.0],
                            color: icon_color,
                        },
                    ]);
                    indices.extend_from_slice(&[index_offset, index_offset + 1, index_offset + 2]);
                    index_offset += 3;
                }
                Tool::Text => {
                    self.add_line(&mut vertices, &mut indices, &mut index_offset,
                        [center[0] - 6.0, center[1] - 8.0], [center[0] + 6.0, center[1] - 8.0], 2.0, icon_color);
                    self.add_line(&mut vertices, &mut indices, &mut index_offset,
                        [center[0], center[1] - 8.0], [center[0], center[1] + 8.0], 2.0, icon_color);
                }
                Tool::Eraser => {
                    vertices.extend_from_slice(&[
                        Vertex {
                            position: [center[0] - 6.0, center[1] - 4.0],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0] + 6.0, center[1] - 4.0],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0] + 6.0, center[1] + 4.0],
                            color: icon_color,
                        },
                        Vertex {
                            position: [center[0] - 6.0, center[1] + 4.0],
                            color: icon_color,
                        },
                    ]);
                    indices.extend_from_slice(&[
                        index_offset, index_offset + 1, index_offset + 2,
                        index_offset, index_offset + 2, index_offset + 3,
                    ]);
                    index_offset += 4;
                }
            }
        }

        // Add zoom indicator in bottom left
        let zoom_percent = (zoom_level * 100.0) as i32;
        let zoom_text_bg_width = 80.0;
        let zoom_text_bg_height = 25.0;
        let zoom_bg_x = 15.0;
        let zoom_bg_y = screen_size.1 - zoom_text_bg_height - 15.0;

        // Zoom indicator background
        let zoom_bg = [
            Vertex {
                position: [zoom_bg_x, zoom_bg_y],
                color: [0.0, 0.0, 0.0, 0.6],
            },
            Vertex {
                position: [zoom_bg_x + zoom_text_bg_width, zoom_bg_y],
                color: [0.0, 0.0, 0.0, 0.6],
            },
            Vertex {
                position: [zoom_bg_x + zoom_text_bg_width, zoom_bg_y + zoom_text_bg_height],
                color: [0.0, 0.0, 0.0, 0.6],
            },
            Vertex {
                position: [zoom_bg_x, zoom_bg_y + zoom_text_bg_height],
                color: [0.0, 0.0, 0.0, 0.6],
            },
        ];
        vertices.extend_from_slice(&zoom_bg);
        indices.extend_from_slice(&[
            index_offset, index_offset + 1, index_offset + 2,
            index_offset, index_offset + 2, index_offset + 3,
        ]);

        (vertices, indices)
    }

    fn add_line(
        &self,
        vertices: &mut Vec<Vertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        start: [f32; 2],
        end: [f32; 2],
        width: f32,
        color: [f32; 4],
    ) {
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let len = (dx * dx + dy * dy).sqrt();
        
        if len > 0.0 {
            let nx = -dy / len * width * 0.5;
            let ny = dx / len * width * 0.5;
            
            vertices.extend_from_slice(&[
                Vertex {
                    position: [start[0] - nx, start[1] - ny],
                    color,
                },
                Vertex {
                    position: [start[0] + nx, start[1] + ny],
                    color,
                },
                Vertex {
                    position: [end[0] + nx, end[1] + ny],
                    color,
                },
                Vertex {
                    position: [end[0] - nx, end[1] - ny],
                    color,
                },
            ]);
            
            indices.extend_from_slice(&[
                *index_offset, *index_offset + 1, *index_offset + 2,
                *index_offset, *index_offset + 2, *index_offset + 3,
            ]);
            *index_offset += 4;
        }
    }

    pub fn handle_click(&self, mouse_pos: [f32; 2], screen_size: (f32, f32)) -> Option<Tool> {
        // Calculate the same positioning logic as in generate_ui_vertices
        let panel_width = 350.0;
        let icon_size = 40.0;
        let num_icons = self.tool_icons.len() as f32;
        let total_icon_width = num_icons * icon_size;
        let spacing = (panel_width - total_icon_width - 20.0) / (num_icons - 1.0);
        let icon_start_x = (screen_size.0 - panel_width) / 2.0 + 10.0;
        let icon_y = 15.0 + 5.0; // panel_top + padding
        
        for (i, icon) in self.tool_icons.iter().enumerate() {
            let icon_x = icon_start_x + i as f32 * (icon_size + spacing);
            
            if mouse_pos[0] >= icon_x
                && mouse_pos[0] <= icon_x + icon_size
                && mouse_pos[1] >= icon_y
                && mouse_pos[1] <= icon_y + icon_size
            {
                return Some(icon.tool);
            }
        }
        None
    }
    
    pub fn is_mouse_over_ui(&self, mouse_pos: [f32; 2], screen_size: (f32, f32)) -> bool {
        // Check if mouse is over the entire toolbar panel
        let panel_width = 350.0;
        let panel_height = 50.0;
        let panel_left = (screen_size.0 - panel_width) / 2.0;
        let panel_right = panel_left + panel_width;
        let panel_top = 15.0;
        let panel_bottom = panel_top + panel_height;
        
        mouse_pos[0] >= panel_left && mouse_pos[0] <= panel_right
            && mouse_pos[1] >= panel_top && mouse_pos[1] <= panel_bottom
    }
} 