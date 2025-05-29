use crate::{Tool, Vertex};

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

    pub fn generate_ui_vertices(&self, current_tool: Tool) -> (Vec<Vertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        let toolbar_bg = [
            Vertex {
                position: [5.0, 5.0],
                color: [0.95, 0.95, 0.95, 0.9],
            },
            Vertex {
                position: [365.0, 5.0],
                color: [0.95, 0.95, 0.95, 0.9],
            },
            Vertex {
                position: [365.0, 55.0],
                color: [0.95, 0.95, 0.95, 0.9],
            },
            Vertex {
                position: [5.0, 55.0],
                color: [0.95, 0.95, 0.95, 0.9],
            },
        ];
        vertices.extend_from_slice(&toolbar_bg);
        indices.extend_from_slice(&[
            index_offset, index_offset + 1, index_offset + 2,
            index_offset, index_offset + 2, index_offset + 3,
        ]);
        index_offset += 4;

        for icon in &self.tool_icons {
            let is_selected = icon.tool == current_tool;
            let color = if is_selected {
                [0.5, 0.7, 1.0, 1.0] 
            } else {
                [0.8, 0.8, 0.8, 1.0]
            };

            vertices.push(Vertex {
                position: icon.position,
                color,
            });
            vertices.push(Vertex {
                position: [icon.position[0] + icon.size[0], icon.position[1]],
                color,
            });
            vertices.push(Vertex {
                position: [icon.position[0] + icon.size[0], icon.position[1] + icon.size[1]],
                color,
            });
            vertices.push(Vertex {
                position: [icon.position[0], icon.position[1] + icon.size[1]],
                color,
            });

            indices.extend_from_slice(&[
                index_offset, index_offset + 1, index_offset + 2,
                index_offset, index_offset + 2, index_offset + 3,
            ]);
            index_offset += 4;

            let icon_color = [0.2, 0.2, 0.2, 1.0];
            let center = [
                icon.position[0] + icon.size[0] / 2.0,
                icon.position[1] + icon.size[1] / 2.0,
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

        let help_y = 65.0;
        let help_bg = [
            Vertex {
                position: [5.0, help_y],
                color: [0.0, 0.0, 0.0, 0.7],
            },
            Vertex {
                position: [400.0, help_y],
                color: [0.0, 0.0, 0.0, 0.7],
            },
            Vertex {
                position: [400.0, help_y + 25.0],
                color: [0.0, 0.0, 0.0, 0.7],
            },
            Vertex {
                position: [5.0, help_y + 25.0],
                color: [0.0, 0.0, 0.0, 0.7],
            },
        ];
        vertices.extend_from_slice(&help_bg);
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

    pub fn handle_click(&self, mouse_pos: [f32; 2]) -> Option<Tool> {
        for icon in &self.tool_icons {
            if mouse_pos[0] >= icon.position[0]
                && mouse_pos[0] <= icon.position[0] + icon.size[0]
                && mouse_pos[1] >= icon.position[1]
                && mouse_pos[1] <= icon.position[1] + icon.size[1]
            {
                return Some(icon.tool);
            }
        }
        None
    }
} 