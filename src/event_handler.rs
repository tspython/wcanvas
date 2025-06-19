use crate::app_state::State;
use crate::drawing::{DrawingElement, Tool};
use crate::state::UserInputState::{Dragging, Drawing, Idle, Panning};

use winit::event::*;

impl<'a> State<'a> {
    pub fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.gpu.config.width = new_size.width;
            self.gpu.config.height = new_size.height;
            self.gpu
                .surface
                .configure(&self.gpu.device, &self.gpu.config);

            self.canvas.uniform.update_transform(
                &self.canvas.transform,
                (new_size.width as f32, new_size.height as f32),
            );
            self.gpu.queue.write_buffer(
                &self.canvas.uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.canvas.uniform]),
            );

            let ui_screen_uniforms = crate::state::UiScreenUniforms {
                screen_size: [new_size.width as f32, new_size.height as f32],
                _padding: [0.0, 0.0],
            };
            self.gpu.queue.write_buffer(
                &self.ui_screen.uniform,
                0,
                bytemuck::cast_slice(&[ui_screen_uniforms]),
            );
        }
    }

    pub fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.input.modifiers = modifiers.state();
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                match button {
                    MouseButton::Left => {
                        match state {
                            ElementState::Pressed => {
                                if self.typing.active {
                                    if !self.typing.buffer.is_empty() {
                                        self.elements.push(DrawingElement::Text {
                                            position: self.typing.pos_canvas,
                                            content: self.typing.buffer.clone(),
                                            color: self.current_color,
                                            size: 32.0,
                                        });
                                    }
                                    self.typing.active = false;
                                    self.typing.buffer.clear();
                                }

                                if let Some(tool) = self.ui_renderer.handle_click(
                                    self.input.mouse_pos, 
                                    (self.size.width as f32, self.size.height as f32)
                                ) {
                                    self.current_tool = tool;
                                    return true;
                                }
                                
                                if self.ui_renderer.is_mouse_over_ui(
                                    self.input.mouse_pos, 
                                    (self.size.width as f32, self.size.height as f32)
                                ) {
                                    return true;
                                }

                                if self.input.modifiers.shift_key() {
                                    self.input.state = Panning;
                                    self.input.pan_start =
                                        Some((self.input.mouse_pos, self.canvas.transform.offset));
                                } else {
                                    let canvas_pos = self
                                        .canvas
                                        .transform
                                        .screen_to_canvas(self.input.mouse_pos);

                                    match self.current_tool {
                                        Tool::Select => {
                                            for (i, element) in self.elements.iter().enumerate() {
                                                if let DrawingElement::Text { position, .. } = element {
                                                    let distance = ((canvas_pos[0] - position[0]).powi(2) + 
                                                                  (canvas_pos[1] - position[1]).powi(2)).sqrt();
                                                    if distance < 50.0 { 
                                                        self.input.state = Dragging;
                                                        self.input.selected_element = Some(i);
                                                        self.input.drag_start = Some(canvas_pos);
                                                        self.input.element_start_pos = Some(*position);
                                                        return true;
                                                    }
                                                }
                                            }
                                        }
                                        _ => {
                                            self.input.state = Drawing;
                                        }
                                    }

                                    match self.current_tool {
                                        Tool::Pen => {
                                            self.input.current_stroke.clear();
                                            self.input.current_stroke.push(canvas_pos);
                                        }
                                        Tool::Rectangle | Tool::Circle | Tool::Arrow => {
                                            self.input.drag_start = Some(canvas_pos);
                                        }
                                        Tool::Text => {
                                            let canvas_pos = self
                                                .canvas
                                                .transform
                                                .screen_to_canvas(self.input.mouse_pos);
                                            self.typing.active = true;
                                            self.typing.pos_canvas = canvas_pos;
                                            self.typing.buffer.clear();
                                            self.typing.cursor_visible = true;
                                            self.typing.blink_timer = std::time::Instant::now();
                                            return true;
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            ElementState::Released => match self.input.state {
                                Panning => {
                                    self.input.state = Idle;
                                    self.input.pan_start = None;
                                }
                                Drawing => {
                                    self.input.state = Idle;
                                    self.finish_drawing();
                                }
                                Dragging => {
                                    self.input.state = Idle;
                                    self.input.selected_element = None;
                                    self.input.drag_start = None;
                                    self.input.element_start_pos = None;
                                }
                                _ => {}
                            },
                        }
                        true
                    }
                    MouseButton::Middle => {
                        match state {
                            ElementState::Pressed => {
                                self.input.state = Panning;
                                self.input.pan_start =
                                    Some((self.input.mouse_pos, self.canvas.transform.offset));
                            }
                            ElementState::Released => {
                                self.input.state = Idle;
                                self.input.pan_start = None;
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = [position.x as f32, position.y as f32];

                if self.input.state == Panning {
                    if let Some((start_mouse, start_offset)) = self.input.pan_start {
                        self.canvas.transform.offset[0] =
                            start_offset[0] + (self.input.mouse_pos[0] - start_mouse[0]);
                        self.canvas.transform.offset[1] =
                            start_offset[1] + (self.input.mouse_pos[1] - start_mouse[1]);

                        self.canvas.uniform.update_transform(
                            &self.canvas.transform,
                            (self.size.width as f32, self.size.height as f32),
                        );
                        self.gpu.queue.write_buffer(
                            &self.canvas.uniform_buffer,
                            0,
                            bytemuck::cast_slice(&[self.canvas.uniform]),
                        );
                    }
                } else if self.input.state == Drawing && self.current_tool == Tool::Pen {
                    if self.ui_renderer.is_mouse_over_ui(
                        self.input.mouse_pos, 
                        (self.size.width as f32, self.size.height as f32)
                    ) {
                        self.finish_drawing();
                        self.input.state = crate::state::UserInputState::Idle;
                        return true;
                    }
                    
                    let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    self.input.current_stroke.push(canvas_pos);
                } else if self.input.state == Dragging {
                    let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    if let (Some(idx), Some(start_mouse), Some(orig_pos)) =
                        (self.input.selected_element, self.input.drag_start, self.input.element_start_pos) {
                        let dx = canvas_pos[0] - start_mouse[0];
                        let dy = canvas_pos[1] - start_mouse[1];
                        if let DrawingElement::Text { position, .. } = &mut self.elements[idx] {
                            position[0] = orig_pos[0] + dx;
                            position[1] = orig_pos[1] + dy;
                        }
                    }
                    return true;
                }
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_factor = match delta {
                    MouseScrollDelta::LineDelta(_, y) => 1.0 + y * 0.1,
                    MouseScrollDelta::PixelDelta(pos) => 1.0 + pos.y as f32 * 0.001,
                };

                let mouse_canvas_before =
                    self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                self.canvas.transform.scale *= zoom_factor;
                self.canvas.transform.scale = self.canvas.transform.scale.clamp(0.1, 10.0);
                let mouse_canvas_after =
                    self.canvas.transform.screen_to_canvas(self.input.mouse_pos);

                self.canvas.transform.offset[0] +=
                    (mouse_canvas_after[0] - mouse_canvas_before[0]) * self.canvas.transform.scale;
                self.canvas.transform.offset[1] +=
                    (mouse_canvas_after[1] - mouse_canvas_before[1]) * self.canvas.transform.scale;

                self.canvas.uniform.update_transform(
                    &self.canvas.transform,
                    (self.size.width as f32, self.size.height as f32),
                );
                self.gpu.queue.write_buffer(
                    &self.canvas.uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[self.canvas.uniform]),
                );

                true
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => {
                if key_event.state != ElementState::Pressed {
                    return false;
                }

                if self.typing.active {
                    if let Some(txt) = &key_event.text {
                        let mut added_visible = false;
                        for ch in txt.chars() {
                            if !ch.is_control() {
                                self.typing.buffer.push(ch);
                                added_visible = true;
                            }
                        }

                        if added_visible {
                            self.typing.cursor_visible = true;
                            self.typing.blink_timer = std::time::Instant::now();
                            return true;
                        }
                    }
                }

                let is_ctrl_or_cmd =
                    self.input.modifiers.control_key() || self.input.modifiers.super_key();

                let keycode_opt = match key_event.physical_key {
                    winit::keyboard::PhysicalKey::Code(code) => Some(code),
                    _ => None,
                };

                if let Some(keycode) = keycode_opt {
                    match keycode {
                        winit::keyboard::KeyCode::Backspace => {
                            if self.typing.active && !self.typing.buffer.is_empty() {
                                self.typing.buffer.pop();
                                return true;
                            }
                            false
                        }
                        winit::keyboard::KeyCode::Enter => {
                            if self.typing.active {
                                if !self.typing.buffer.is_empty() {
                                    self.elements.push(DrawingElement::Text {
                                        position: self.typing.pos_canvas,
                                        content: self.typing.buffer.clone(),
                                        color: self.current_color,
                                        size: 32.0,
                                    });
                                }
                                self.typing.active = false;
                                self.typing.buffer.clear();
                                return true;
                            }
                            false
                        }
                        winit::keyboard::KeyCode::Digit1 => {
                            self.current_tool = Tool::Select;
                            true
                        }
                        winit::keyboard::KeyCode::Digit2 => {
                            self.current_tool = Tool::Pen;
                            true
                        }
                        winit::keyboard::KeyCode::Digit3 => {
                            self.current_tool = Tool::Rectangle;
                            true
                        }
                        winit::keyboard::KeyCode::Digit4 => {
                            self.current_tool = Tool::Circle;
                            true
                        }
                        winit::keyboard::KeyCode::Digit5 => {
                            self.current_tool = Tool::Arrow;
                            true
                        }
                        winit::keyboard::KeyCode::Digit6 => {
                            self.current_tool = Tool::Text;
                            true
                        }
                        winit::keyboard::KeyCode::Minus => {
                            if is_ctrl_or_cmd {
                                self.canvas.transform.scale *= 0.9;
                                self.canvas.transform.scale =
                                    self.canvas.transform.scale.clamp(0.1, 10.0);
                                self.canvas.uniform.update_transform(
                                    &self.canvas.transform,
                                    (self.size.width as f32, self.size.height as f32),
                                );
                                self.gpu.queue.write_buffer(
                                    &self.canvas.uniform_buffer,
                                    0,
                                    bytemuck::cast_slice(&[self.canvas.uniform]),
                                );
                                true
                            } else {
                                false
                            }
                        }
                        winit::keyboard::KeyCode::Equal => {
                            if is_ctrl_or_cmd {
                                self.canvas.transform.scale *= 1.1;
                                self.canvas.transform.scale =
                                    self.canvas.transform.scale.clamp(0.1, 10.0);
                                self.canvas.uniform.update_transform(
                                    &self.canvas.transform,
                                    (self.size.width as f32, self.size.height as f32),
                                );
                                self.gpu.queue.write_buffer(
                                    &self.canvas.uniform_buffer,
                                    0,
                                    bytemuck::cast_slice(&[self.canvas.uniform]),
                                );
                                true
                            } else {
                                false
                            }
                        }
                        _ => false,
                    }
                } else {
                    false
                }
            }
            WindowEvent::Ime(ime) => {
                if let winit::event::Ime::Commit(text) = ime {
                    if self.typing.active {
                        for ch in text.chars() {
                            if !ch.is_control() {
                                self.typing.buffer.push(ch);
                            }
                        }
                        return true;
                    }
                }
                false
            }
            _ => false,
        }
    }

    pub fn finish_drawing(&mut self) {
        let element = match self.current_tool {
            Tool::Pen => {
                if self.input.current_stroke.len() > 1 {
                    Some(DrawingElement::Stroke {
                        points: self.input.current_stroke.clone(),
                        color: self.current_color,
                        width: self.stroke_width,
                    })
                } else {
                    None
                }
            }
            Tool::Rectangle => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let position = [start[0].min(end[0]), start[1].min(end[1])];
                    let size = [(end[0] - start[0]).abs(), (end[1] - start[1]).abs()];

                    Some(DrawingElement::Rectangle {
                        position,
                        size,
                        color: self.current_color,
                        fill: false,
                        stroke_width: self.stroke_width,
                    })
                } else {
                    None
                }
            }
            Tool::Circle => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let radius = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();

                    Some(DrawingElement::Circle {
                        center: start,
                        radius,
                        color: self.current_color,
                        fill: false,
                        stroke_width: self.stroke_width,
                    })
                } else {
                    None
                }
            }
            Tool::Arrow => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);

                    Some(DrawingElement::Arrow {
                        start,
                        end,
                        color: self.current_color,
                        width: self.stroke_width,
                    })
                } else {
                    None
                }
            }
            _ => None,
        };

        if let Some(element) = element {
            self.elements.push(element);
        }

        self.input.current_stroke.clear();
        self.input.drag_start = None;
    }
}
