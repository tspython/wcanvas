use crate::app_state::State;
use crate::drawing::DrawingElement;
use crate::vertex::Vertex;
use wgpu::util::DeviceExt;

impl State {
    pub fn update(&mut self) {
        if self.typing.active {
            let elapsed = self.typing.blink_timer.elapsed();
            if elapsed.as_millis() > 500 {
                self.typing.cursor_visible = !self.typing.cursor_visible;
                cfg_if::cfg_if! {
                    if #[cfg(target_arch = "wasm32")] {
                        self.typing.blink_timer = web_time::Instant::now();
                    } else {
                        self.typing.blink_timer = std::time::Instant::now();
                    }
                }
            }
        }
        self.update_buffers();

        let (ui_vertices, ui_indices) = self.ui_renderer.generate_ui_vertices(
            self.current_tool,
            self.current_color,
            (self.size.width as f32, self.size.height as f32),
            self.canvas.transform.scale
        );

        if !ui_vertices.is_empty() {
            self.ui_geo.vertex = Some(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("UI Vertex Buffer"),
                    contents: bytemuck::cast_slice(&ui_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));

            self.ui_geo.index = Some(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("UI Index Buffer"),
                    contents: bytemuck::cast_slice(&ui_indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));

            self.ui_geo.count = ui_indices.len() as u32;
        }

        let mut drawing_elements = self.elements.clone();
        if self.typing.active {
            let mut display_text = self.typing.buffer.clone();
            if self.typing.cursor_visible {
                display_text.push('|');
            }
            drawing_elements.push(DrawingElement::Text {
                position: self.typing.pos_canvas,
                content: display_text,
                color: self.current_color,
                size: 32.0,
            });
        }
        
        self.text_renderer.prepare(
            &self.gpu.device,
            &self.gpu.queue,
            &drawing_elements,
            (self.size.width as f32, self.size.height as f32),
        );

        self.text_renderer.clear_screen();
        let zoom_percent = (self.canvas.transform.scale * 100.0) as i32;
        let zoom_text = format!("{}%", zoom_percent);
        let screen_pos = [25.0, self.size.height as f32 - 25.0];
        self.text_renderer.add_screen_label(
            &self.gpu.device,
            &self.gpu.queue,
            &zoom_text,
            screen_pos,
            14.0,
            [1.0, 1.0, 1.0, 1.0],
        );
        
        self.text_renderer.build_screen_buffers(&self.gpu.device);
    }

    fn update_buffers(&mut self) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        for (element_index, element) in self.elements.iter().enumerate() {
            match element {
                DrawingElement::Stroke {
                    points,
                    color,
                    width,
                } => {
                    for i in 0..points.len().saturating_sub(1) {
                        let p1 = points[i];
                        let p2 = points[i + 1];

                        let dx = p2[0] - p1[0];
                        let dy = p2[1] - p1[1];
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.0 {
                            let nx = -dy / len * width * 0.5;
                            let ny = dx / len * width * 0.5;

                            vertices.push(Vertex {
                                position: [p1[0] - nx, p1[1] - ny],
                                color: *color,
                            });
                            vertices.push(Vertex {
                                position: [p1[0] + nx, p1[1] + ny],
                                color: *color,
                            });
                            vertices.push(Vertex {
                                position: [p2[0] + nx, p2[1] + ny],
                                color: *color,
                            });
                            vertices.push(Vertex {
                                position: [p2[0] - nx, p2[1] - ny],
                                color: *color,
                            });

                            indices.extend_from_slice(&[
                                index_offset,
                                index_offset + 1,
                                index_offset + 2,
                                index_offset,
                                index_offset + 2,
                                index_offset + 3,
                            ]);
                            index_offset += 4;
                        }
                    }
                }
                DrawingElement::Rectangle {
                    position,
                    size,
                    color,
                    fill,
                    stroke_width,
                } => {
                    if *fill {
                        vertices.push(Vertex {
                            position: *position,
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [position[0] + size[0], position[1]],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [position[0] + size[0], position[1] + size[1]],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [position[0], position[1] + size[1]],
                            color: *color,
                        });

                        indices.extend_from_slice(&[
                            index_offset,
                            index_offset + 1,
                            index_offset + 2,
                            index_offset,
                            index_offset + 2,
                            index_offset + 3,
                        ]);
                        index_offset += 4;
                    } else {
                        let corners = [
                            *position,
                            [position[0] + size[0], position[1]],
                            [position[0] + size[0], position[1] + size[1]],
                            [position[0], position[1] + size[1]],
                        ];

                        for i in 0..4 {
                            let p1 = corners[i];
                            let p2 = corners[(i + 1) % 4];

                            let dx = p2[0] - p1[0];
                            let dy = p2[1] - p1[1];
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 0.0 {
                                let nx = -dy / len * stroke_width * 0.5;
                                let ny = dx / len * stroke_width * 0.5;

                                vertices.push(Vertex {
                                    position: [p1[0] - nx, p1[1] - ny],
                                    color: *color,
                                });
                                vertices.push(Vertex {
                                    position: [p1[0] + nx, p1[1] + ny],
                                    color: *color,
                                });
                                vertices.push(Vertex {
                                    position: [p2[0] + nx, p2[1] + ny],
                                    color: *color,
                                });
                                vertices.push(Vertex {
                                    position: [p2[0] - nx, p2[1] - ny],
                                    color: *color,
                                });

                                indices.extend_from_slice(&[
                                    index_offset,
                                    index_offset + 1,
                                    index_offset + 2,
                                    index_offset,
                                    index_offset + 2,
                                    index_offset + 3,
                                ]);
                                index_offset += 4;
                            }
                        }
                    }
                }
                DrawingElement::Circle {
                    center,
                    radius,
                    color,
                    fill,
                    stroke_width,
                } => {
                    const SEGMENTS: u32 = 32;

                    if *fill {
                        let center_index = index_offset;
                        vertices.push(Vertex {
                            position: *center,
                            color: *color,
                        });
                        index_offset += 1;

                        for i in 0..SEGMENTS {
                            let angle = (i as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                            vertices.push(Vertex {
                                position: [
                                    center[0] + angle.cos() * radius,
                                    center[1] + angle.sin() * radius,
                                ],
                                color: *color,
                            });
                        }

                        for i in 0..SEGMENTS {
                            indices.extend_from_slice(&[
                                center_index,
                                center_index + 1 + i as u16,
                                center_index + 1 + ((i + 1) % SEGMENTS) as u16,
                            ]);
                        }
                        index_offset += SEGMENTS as u16;
                    } else {
                        for i in 0..SEGMENTS {
                            let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                            let angle2 =
                                ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;

                            let p1 = [
                                center[0] + angle1.cos() * radius,
                                center[1] + angle1.sin() * radius,
                            ];
                            let p2 = [
                                center[0] + angle2.cos() * radius,
                                center[1] + angle2.sin() * radius,
                            ];

                            // Create thick line segment
                            let dx = p2[0] - p1[0];
                            let dy = p2[1] - p1[1];
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 0.0 {
                                let nx = -dy / len * stroke_width * 0.5;
                                let ny = dx / len * stroke_width * 0.5;

                                vertices.push(Vertex {
                                    position: [p1[0] - nx, p1[1] - ny],
                                    color: *color,
                                });
                                vertices.push(Vertex {
                                    position: [p1[0] + nx, p1[1] + ny],
                                    color: *color,
                                });
                                vertices.push(Vertex {
                                    position: [p2[0] + nx, p2[1] + ny],
                                    color: *color,
                                });
                                vertices.push(Vertex {
                                    position: [p2[0] - nx, p2[1] - ny],
                                    color: *color,
                                });

                                indices.extend_from_slice(&[
                                    index_offset,
                                    index_offset + 1,
                                    index_offset + 2,
                                    index_offset,
                                    index_offset + 2,
                                    index_offset + 3,
                                ]);
                                index_offset += 4;
                            }
                        }
                    }
                }
                DrawingElement::Arrow {
                    start,
                    end,
                    color,
                    width,
                } => {
                    let dx = end[0] - start[0];
                    let dy = end[1] - start[1];
                    let len = (dx * dx + dy * dy).sqrt();

                    if len > 0.0 {
                        let nx = -dy / len * width * 0.5;
                        let ny = dx / len * width * 0.5;

                        vertices.push(Vertex {
                            position: [start[0] - nx, start[1] - ny],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [start[0] + nx, start[1] + ny],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [end[0] + nx, end[1] + ny],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [end[0] - nx, end[1] - ny],
                            color: *color,
                        });

                        indices.extend_from_slice(&[
                            index_offset,
                            index_offset + 1,
                            index_offset + 2,
                            index_offset,
                            index_offset + 2,
                            index_offset + 3,
                        ]);
                        index_offset += 4;

                        let head_len = 20.0;
                        let head_angle = 0.5; 

                        let dir_x = dx / len;
                        let dir_y = dy / len;

                        let cos_angle = (head_angle as f32).cos();
                        let sin_angle = (head_angle as f32).sin();

                        let left_x = end[0] - head_len * (dir_x * cos_angle - dir_y * sin_angle);
                        let left_y = end[1] - head_len * (dir_y * cos_angle + dir_x * sin_angle);

                        let right_x = end[0] - head_len * (dir_x * cos_angle + dir_y * sin_angle);
                        let right_y = end[1] - head_len * (dir_y * cos_angle - dir_x * sin_angle);

                        let nx1 = -(left_y - end[1]) / head_len * width * 0.5;
                        let ny1 = (left_x - end[0]) / head_len * width * 0.5;

                        vertices.push(Vertex {
                            position: [end[0] - nx1, end[1] - ny1],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [end[0] + nx1, end[1] + ny1],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [left_x + nx1, left_y + ny1],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [left_x - nx1, left_y - ny1],
                            color: *color,
                        });

                        indices.extend_from_slice(&[
                            index_offset,
                            index_offset + 1,
                            index_offset + 2,
                            index_offset,
                            index_offset + 2,
                            index_offset + 3,
                        ]);
                        index_offset += 4;

                        let nx2 = -(right_y - end[1]) / head_len * width * 0.5;
                        let ny2 = (right_x - end[0]) / head_len * width * 0.5;

                        vertices.push(Vertex {
                            position: [end[0] - nx2, end[1] - ny2],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [end[0] + nx2, end[1] + ny2],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [right_x + nx2, right_y + ny2],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [right_x - nx2, right_y - ny2],
                            color: *color,
                        });

                        indices.extend_from_slice(&[
                            index_offset,
                            index_offset + 1,
                            index_offset + 2,
                            index_offset,
                            index_offset + 2,
                            index_offset + 3,
                        ]);
                        index_offset += 4;
                    }
                }
                DrawingElement::Text { .. } => {}
                DrawingElement::TextBox { .. } => {}
            }
        }

        if let Some(selected_idx) = self.input.selected_element {
            if let Some(element) = self.elements.get(selected_idx) {
                self.add_selection_highlight(element, &mut vertices, &mut indices, &mut index_offset);
            }
        }

        if self.input.state == crate::state::UserInputState::Drawing {
            match self.current_tool {
                crate::drawing::Tool::Pen => {
                    if self.input.current_stroke.len() > 1 {
                        for i in 0..self.input.current_stroke.len().saturating_sub(1) {
                            let p1 = self.input.current_stroke[i];
                            let p2 = self.input.current_stroke[i + 1];

                            let dx = p2[0] - p1[0];
                            let dy = p2[1] - p1[1];
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 0.0 {
                                let nx = -dy / len * self.stroke_width * 0.5;
                                let ny = dx / len * self.stroke_width * 0.5;

                                vertices.push(Vertex {
                                    position: [p1[0] - nx, p1[1] - ny],
                                    color: self.current_color,
                                });
                                vertices.push(Vertex {
                                    position: [p1[0] + nx, p1[1] + ny],
                                    color: self.current_color,
                                });
                                vertices.push(Vertex {
                                    position: [p2[0] + nx, p2[1] + ny],
                                    color: self.current_color,
                                });
                                vertices.push(Vertex {
                                    position: [p2[0] - nx, p2[1] - ny],
                                    color: self.current_color,
                                });

                                indices.extend_from_slice(&[
                                    index_offset,
                                    index_offset + 1,
                                    index_offset + 2,
                                    index_offset,
                                    index_offset + 2,
                                    index_offset + 3,
                                ]);
                                index_offset += 4;
                            }
                        }
                    }
                }
                crate::drawing::Tool::Arrow => {
                    if let Some(start) = self.input.drag_start {
                        let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                        let dx = end[0] - start[0];
                        let dy = end[1] - start[1];
                        let len = (dx * dx + dy * dy).sqrt();

                        if len > 0.0 {
                            let nx = -dy / len * self.stroke_width * 0.5;
                            let ny = dx / len * self.stroke_width * 0.5;

                            vertices.push(Vertex {
                                position: [start[0] - nx, start[1] - ny],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [start[0] + nx, start[1] + ny],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [end[0] + nx, end[1] + ny],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [end[0] - nx, end[1] - ny],
                                color: self.current_color,
                            });

                            indices.extend_from_slice(&[
                                index_offset,
                                index_offset + 1,
                                index_offset + 2,
                                index_offset,
                                index_offset + 2,
                                index_offset + 3,
                            ]);
                            index_offset += 4;

                            let head_len = 20.0;
                            let head_angle = 0.5_f32;

                            let dir_x = dx / len;
                            let dir_y = dy / len;

                            let cos_angle = head_angle.cos();
                            let sin_angle = head_angle.sin();

                            let left_x = end[0] - head_len * (dir_x * cos_angle - dir_y * sin_angle);
                            let left_y = end[1] - head_len * (dir_y * cos_angle + dir_x * sin_angle);

                            let right_x = end[0] - head_len * (dir_x * cos_angle + dir_y * sin_angle);
                            let right_y = end[1] - head_len * (dir_y * cos_angle - dir_x * sin_angle);

                            let nx1 = -(left_y - end[1]) / head_len * self.stroke_width * 0.5;
                            let ny1 = (left_x - end[0]) / head_len * self.stroke_width * 0.5;

                            vertices.push(Vertex {
                                position: [end[0] - nx1, end[1] - ny1],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [end[0] + nx1, end[1] + ny1],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [left_x + nx1, left_y + ny1],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [left_x - nx1, left_y - ny1],
                                color: self.current_color,
                            });

                            indices.extend_from_slice(&[
                                index_offset,
                                index_offset + 1,
                                index_offset + 2,
                                index_offset,
                                index_offset + 2,
                                index_offset + 3,
                            ]);
                            index_offset += 4;

                            let nx2 = -(right_y - end[1]) / head_len * self.stroke_width * 0.5;
                            let ny2 = (right_x - end[0]) / head_len * self.stroke_width * 0.5;

                            vertices.push(Vertex {
                                position: [end[0] - nx2, end[1] - ny2],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [end[0] + nx2, end[1] + ny2],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [right_x + nx2, right_y + ny2],
                                color: self.current_color,
                            });
                            vertices.push(Vertex {
                                position: [right_x - nx2, right_y - ny2],
                                color: self.current_color,
                            });

                            indices.extend_from_slice(&[
                                index_offset,
                                index_offset + 1,
                                index_offset + 2,
                                index_offset,
                                index_offset + 2,
                                index_offset + 3,
                            ]);
                            index_offset += 4;
                        }
                    }
                }
                _ => {
                    // TODO: Implement preview for other tools
                }
            }
        }

        if !vertices.is_empty() {
            self.geometry.vertex = Some(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));

            self.geometry.index = Some(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));

            self.geometry.count = indices.len() as u32;
        } else {
            self.geometry.vertex = None;
            self.geometry.index = None;
            self.geometry.count = 0;
        }
    }
    
    fn add_selection_highlight(&self, element: &crate::drawing::DrawingElement, vertices: &mut Vec<crate::vertex::Vertex>, indices: &mut Vec<u16>, index_offset: &mut u16) {
        let selection_color = [0.0, 0.5, 1.0, 0.8]; 
        let highlight_width = 3.0;
        
        match element {
            crate::drawing::DrawingElement::Text { position, .. } => {
                let radius = 30.0;
                const SEGMENTS: u32 = 16;
                
                for i in 0..SEGMENTS {
                    let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                    let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                    
                    let p1 = [
                        position[0] + angle1.cos() * radius,
                        position[1] + angle1.sin() * radius,
                    ];
                    let p2 = [
                        position[0] + angle2.cos() * radius,
                        position[1] + angle2.sin() * radius,
                    ];
                    
                    let dx = p2[0] - p1[0];
                    let dy = p2[1] - p1[1];
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let nx = -dy / len * highlight_width * 0.5;
                        let ny = dx / len * highlight_width * 0.5;
                        
                        vertices.push(crate::vertex::Vertex {
                            position: [p1[0] - nx, p1[1] - ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p1[0] + nx, p1[1] + ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p2[0] + nx, p2[1] + ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p2[0] - nx, p2[1] - ny],
                            color: selection_color,
                        });
                        
                        indices.extend_from_slice(&[
                            *index_offset,
                            *index_offset + 1,
                            *index_offset + 2,
                            *index_offset,
                            *index_offset + 2,
                            *index_offset + 3,
                        ]);
                        *index_offset += 4;
                    }
                }
            }
            crate::drawing::DrawingElement::Rectangle { position, size, .. } => {
                let margin = 5.0;
                let expanded_pos = [position[0] - margin, position[1] - margin];
                let expanded_size = [size[0] + margin * 2.0, size[1] + margin * 2.0];
                
                let corners = [
                    expanded_pos,
                    [expanded_pos[0] + expanded_size[0], expanded_pos[1]],
                    [expanded_pos[0] + expanded_size[0], expanded_pos[1] + expanded_size[1]],
                    [expanded_pos[0], expanded_pos[1] + expanded_size[1]],
                ];
                
                for i in 0..4 {
                    let p1 = corners[i];
                    let p2 = corners[(i + 1) % 4];
                    
                    let dx = p2[0] - p1[0];
                    let dy = p2[1] - p1[1];
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let nx = -dy / len * highlight_width * 0.5;
                        let ny = dx / len * highlight_width * 0.5;
                        
                        vertices.push(crate::vertex::Vertex {
                            position: [p1[0] - nx, p1[1] - ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p1[0] + nx, p1[1] + ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p2[0] + nx, p2[1] + ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p2[0] - nx, p2[1] - ny],
                            color: selection_color,
                        });
                        
                        indices.extend_from_slice(&[
                            *index_offset,
                            *index_offset + 1,
                            *index_offset + 2,
                            *index_offset,
                            *index_offset + 2,
                            *index_offset + 3,
                        ]);
                        *index_offset += 4;
                    }
                }
            }
            crate::drawing::DrawingElement::Circle { center, radius, .. } => {
                // Draw selection circle around the original circle
                let expanded_radius = radius + 10.0;
                const SEGMENTS: u32 = 32;
                
                for i in 0..SEGMENTS {
                    let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                    let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                    
                    let p1 = [
                        center[0] + angle1.cos() * expanded_radius,
                        center[1] + angle1.sin() * expanded_radius,
                    ];
                    let p2 = [
                        center[0] + angle2.cos() * expanded_radius,
                        center[1] + angle2.sin() * expanded_radius,
                    ];
                    
                    let dx = p2[0] - p1[0];
                    let dy = p2[1] - p1[1];
                    let len = (dx * dx + dy * dy).sqrt();
                    if len > 0.0 {
                        let nx = -dy / len * highlight_width * 0.5;
                        let ny = dx / len * highlight_width * 0.5;
                        
                        vertices.push(crate::vertex::Vertex {
                            position: [p1[0] - nx, p1[1] - ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p1[0] + nx, p1[1] + ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p2[0] + nx, p2[1] + ny],
                            color: selection_color,
                        });
                        vertices.push(crate::vertex::Vertex {
                            position: [p2[0] - nx, p2[1] - ny],
                            color: selection_color,
                        });
                        
                        indices.extend_from_slice(&[
                            *index_offset,
                            *index_offset + 1,
                            *index_offset + 2,
                            *index_offset,
                            *index_offset + 2,
                            *index_offset + 3,
                        ]);
                        *index_offset += 4;
                    }
                }
            }
            crate::drawing::DrawingElement::Arrow { start, end, .. } => {
                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let len = (dx * dx + dy * dy).sqrt();
                
                if len > 0.0 {
                    let expanded_width = highlight_width * 2.0;
                    let nx = -dy / len * expanded_width * 0.5;
                    let ny = dx / len * expanded_width * 0.5;
                    
                    vertices.push(crate::vertex::Vertex {
                        position: [start[0] - nx, start[1] - ny],
                        color: selection_color,
                    });
                    vertices.push(crate::vertex::Vertex {
                        position: [start[0] + nx, start[1] + ny],
                        color: selection_color,
                    });
                    vertices.push(crate::vertex::Vertex {
                        position: [end[0] + nx, end[1] + ny],
                        color: selection_color,
                    });
                    vertices.push(crate::vertex::Vertex {
                        position: [end[0] - nx, end[1] - ny],
                        color: selection_color,
                    });
                    
                    indices.extend_from_slice(&[
                        *index_offset,
                        *index_offset + 1,
                        *index_offset + 2,
                        *index_offset,
                        *index_offset + 2,
                        *index_offset + 3,
                    ]);
                    *index_offset += 4;
                }
            }
            crate::drawing::DrawingElement::Stroke { points, .. } => {
                if points.len() > 1 {
                    for i in 0..points.len().saturating_sub(1) {
                        let p1 = points[i];
                        let p2 = points[i + 1];
                        
                        let dx = p2[0] - p1[0];
                        let dy = p2[1] - p1[1];
                        let len = (dx * dx + dy * dy).sqrt();
                        if len > 0.0 {
                            let expanded_width = highlight_width * 2.0;
                            let nx = -dy / len * expanded_width * 0.5;
                            let ny = dx / len * expanded_width * 0.5;
                            
                            vertices.push(crate::vertex::Vertex {
                                position: [p1[0] - nx, p1[1] - ny],
                                color: selection_color,
                            });
                            vertices.push(crate::vertex::Vertex {
                                position: [p1[0] + nx, p1[1] + ny],
                                color: selection_color,
                            });
                            vertices.push(crate::vertex::Vertex {
                                position: [p2[0] + nx, p2[1] + ny],
                                color: selection_color,
                            });
                            vertices.push(crate::vertex::Vertex {
                                position: [p2[0] - nx, p2[1] - ny],
                                color: selection_color,
                            });
                            
                            indices.extend_from_slice(&[
                                *index_offset,
                                *index_offset + 1,
                                *index_offset + 2,
                                *index_offset,
                                *index_offset + 2,
                                *index_offset + 3,
                            ]);
                            *index_offset += 4;
                        }
                    }
                }
            }
            _ => {}
        }
    }
}
