use crate::app_state::State;
use crate::drawing::{DrawingElement, Tool};
use crate::state::UserInputState::{Dragging, Drawing, Idle, Panning};
use rand::Rng;

use winit::event::*;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use web_time::Instant;
    } else {
        use std::time::Instant;
    }
}

impl State {
    fn is_mouse_in_titlebar(&self, mouse_pos: [f32; 2]) -> bool {
        #[cfg(target_os = "macos")]
        {
            mouse_pos[1] < 22.0
        }
        #[cfg(not(target_os = "macos"))]
        {
            false
        }
    }

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
                                
                                if let Some(color) = self.ui_renderer.handle_color_click(
                                    self.input.mouse_pos
                                ) {
                                    self.current_color = color;
                                    return true;
                                }
                                
                                if self.ui_renderer.is_mouse_over_ui(
                                    self.input.mouse_pos, 
                                    (self.size.width as f32, self.size.height as f32)
                                ) {
                                    return true;
                                }

                                // Prevent drawing in titlebar area on macOS
                                if self.is_mouse_in_titlebar(self.input.mouse_pos) {
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
                                                if self.is_element_at_position(element, canvas_pos) {
                                                    #[cfg(debug_assertions)]
                                                    println!("Selected element {} at position {:?}", i, canvas_pos);
                                                    self.input.state = Dragging;
                                                    self.input.selected_element = Some(i);
                                                    self.input.drag_start = Some(canvas_pos);
                                                    self.input.element_start_pos = Some(self.get_element_position(element));
                                                    return true;
                                                }
                                            }
                                            self.input.selected_element = None;
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
                                            self.typing.blink_timer = Instant::now();
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
                    ) || self.is_mouse_in_titlebar(self.input.mouse_pos) {
                        self.finish_drawing();
                        self.input.state = crate::state::UserInputState::Idle;
                        return true;
                    }
                    
                    let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    self.input.current_stroke.push(canvas_pos);
                } else if self.input.state == Drawing {
                    self.update_preview_element();
                } else if self.input.state == Dragging {
                    let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    if let (Some(idx), Some(start_mouse), Some(orig_pos)) =
                        (self.input.selected_element, self.input.drag_start, self.input.element_start_pos) {
                        let dx = canvas_pos[0] - start_mouse[0];
                        let dy = canvas_pos[1] - start_mouse[1];
                        self.move_element(idx, orig_pos, dx, dy);
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
                            self.typing.blink_timer = Instant::now();
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

                    let mut rough_options = crate::rough::RoughOptions::default();
                    rough_options.stroke_width = self.stroke_width;
                    
                    let mut rng = rand::rng();
                    
                    rough_options.roughness = 0.7 + rng.random::<f32>() * 1.0;
                    rough_options.bowing = 0.3 + rng.random::<f32>() * 1.2;
                    rough_options.max_randomness_offset = 1.0 + rng.random::<f32>() * 1.5;
                    rough_options.curve_tightness = rng.random::<f32>() * 0.2;
                    
                    rough_options.seed = Some(rng.random::<u64>());

                    Some(DrawingElement::Rectangle {
                        position,
                        size,
                        color: self.current_color,
                        fill: false,
                        stroke_width: self.stroke_width,
                        rough_style: Some(rough_options),
                    })
                } else {
                    None
                }
            }
            Tool::Circle => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let radius = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();

                    let mut rough_options = crate::rough::RoughOptions::default();
                    rough_options.stroke_width = self.stroke_width;
                    
                    let mut rng = rand::rng();
                    
                    rough_options.roughness = 0.4 + rng.random::<f32>() * 0.4;
                    rough_options.bowing = 0.2 + rng.random::<f32>() * 0.3;
                    rough_options.max_randomness_offset = 0.5 + rng.random::<f32>() * 0.5;
                    rough_options.curve_step_count = 32 + (rng.random::<f32>() * 8.0) as u32;
                    rough_options.curve_tightness = rng.random::<f32>() * 0.1;
                    
                    rough_options.seed = Some(rng.random::<u64>());

                    Some(DrawingElement::Circle {
                        center: start,
                        radius,
                        color: self.current_color,
                        fill: false,
                        stroke_width: self.stroke_width,
                        rough_style: Some(rough_options),
                    })
                } else {
                    None
                }
            }
            Tool::Arrow => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);

                    let mut rough_options = crate::rough::RoughOptions::default();
                    rough_options.stroke_width = self.stroke_width;
                    
                    let mut rng = rand::rng();
                    
                    rough_options.roughness = 0.5 + rng.random::<f32>() * 0.6;
                    rough_options.bowing = 0.3 + rng.random::<f32>() * 0.4;
                    rough_options.max_randomness_offset = 0.8 + rng.random::<f32>() * 0.7;
                    rough_options.curve_step_count = 8 + (rng.random::<f32>() * 4.0) as u32;
                    rough_options.curve_tightness = rng.random::<f32>() * 0.1;
                    
                    rough_options.seed = Some(rng.random::<u64>());

                    Some(DrawingElement::Arrow {
                        start,
                        end,
                        color: self.current_color,
                        width: self.stroke_width,
                        rough_style: Some(rough_options),
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
        self.input.preview_element = None;
    }

    fn update_preview_element(&mut self) {
        if self.input.state != Drawing {
            self.input.preview_element = None;
            return;
        }

        match self.current_tool {
            Tool::Rectangle => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let position = [start[0].min(end[0]), start[1].min(end[1])];
                    let size = [(end[0] - start[0]).abs(), (end[1] - start[1]).abs()];

                    self.input.preview_element = Some(DrawingElement::Rectangle {
                        position,
                        size,
                        color: [self.current_color[0], self.current_color[1], self.current_color[2], 0.5],
                        fill: false,
                        stroke_width: self.stroke_width,
                        rough_style: None,
                    });
                }
            }
            Tool::Circle => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let radius = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();

                    self.input.preview_element = Some(DrawingElement::Circle {
                        center: start,
                        radius,
                        color: [self.current_color[0], self.current_color[1], self.current_color[2], 0.5],
                        fill: false,
                        stroke_width: self.stroke_width,
                        rough_style: None,
                    });
                }
            }
            Tool::Arrow => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);

                    self.input.preview_element = Some(DrawingElement::Arrow {
                        start,
                        end,
                        color: [self.current_color[0], self.current_color[1], self.current_color[2], 0.5],
                        width: self.stroke_width,
                        rough_style: None,
                    });
                }
            }
            _ => {
                self.input.preview_element = None;
            }
        }
    }

    // Helper methods for element selection and manipulation
    fn is_element_at_position(&self, element: &DrawingElement, pos: [f32; 2]) -> bool {
        match element {
            DrawingElement::Text { position, content, size, .. } => {
                let char_width = size * 0.6;
                let text_width = content.len() as f32 * char_width;
                let text_height = size * 1.2;
                
                pos[0] >= position[0] - 5.0 && pos[0] <= position[0] + text_width + 5.0 &&
                pos[1] >= position[1] - text_height && pos[1] <= position[1] + 5.0
            }
            DrawingElement::TextBox { pos: element_pos, size, .. } => {
                pos[0] >= element_pos[0] && pos[0] <= element_pos[0] + size[0] &&
                pos[1] >= element_pos[1] && pos[1] <= element_pos[1] + size[1]
            }
            DrawingElement::Rectangle { position, size, .. } => {
                pos[0] >= position[0] && pos[0] <= position[0] + size[0] &&
                pos[1] >= position[1] && pos[1] <= position[1] + size[1]
            }
            DrawingElement::Circle { center, radius, .. } => {
                let distance = ((pos[0] - center[0]).powi(2) + (pos[1] - center[1]).powi(2)).sqrt();
                distance <= *radius
            }
            DrawingElement::Arrow { start, end, width, .. } => {
                self.point_to_line_distance(pos, *start, *end) <= width * 2.0
            }
            DrawingElement::Stroke { points, width, .. } => {
                for i in 0..points.len().saturating_sub(1) {
                    if self.point_to_line_distance(pos, points[i], points[i + 1]) <= width * 2.0 {
                        return true;
                    }
                }
                false
            }
        }
    }
    
    fn get_element_position(&self, element: &DrawingElement) -> [f32; 2] {
        match element {
            DrawingElement::Text { position, .. } => *position,
            DrawingElement::TextBox { pos, .. } => *pos,
            DrawingElement::Rectangle { position, .. } => *position,
            DrawingElement::Circle { center, .. } => *center,
            DrawingElement::Arrow { start, .. } => *start,
            DrawingElement::Stroke { points, .. } => {
                if points.is_empty() {
                    [0.0, 0.0]
                } else {
                    points[0]
                }
            }
        }
    }
    
    fn move_element(&mut self, idx: usize, orig_pos: [f32; 2], dx: f32, dy: f32) {
        if let Some(element) = self.elements.get_mut(idx) {
            match element {
                DrawingElement::Text { position, .. } => {
                    position[0] = orig_pos[0] + dx;
                    position[1] = orig_pos[1] + dy;
                }
                DrawingElement::TextBox { pos, .. } => {
                    pos[0] = orig_pos[0] + dx;
                    pos[1] = orig_pos[1] + dy;
                }
                DrawingElement::Rectangle { position, .. } => {
                    position[0] = orig_pos[0] + dx;
                    position[1] = orig_pos[1] + dy;
                }
                DrawingElement::Circle { center, .. } => {
                    center[0] = orig_pos[0] + dx;
                    center[1] = orig_pos[1] + dy;
                }
                DrawingElement::Arrow { start, end, .. } => {
                    let arrow_dx = end[0] - start[0];
                    let arrow_dy = end[1] - start[1];
                    start[0] = orig_pos[0] + dx;
                    start[1] = orig_pos[1] + dy;
                    end[0] = start[0] + arrow_dx;
                    end[1] = start[1] + arrow_dy;
                }
                DrawingElement::Stroke { points, .. } => {
                    if !points.is_empty() {
                        let stroke_dx = orig_pos[0] + dx - points[0][0];
                        let stroke_dy = orig_pos[1] + dy - points[0][1];
                        for point in points.iter_mut() {
                            point[0] += stroke_dx;
                            point[1] += stroke_dy;
                        }
                    }
                }
            }
        }
    }
    
    fn point_to_line_distance(&self, point: [f32; 2], line_start: [f32; 2], line_end: [f32; 2]) -> f32 {
        let line_length_squared = (line_end[0] - line_start[0]).powi(2) + (line_end[1] - line_start[1]).powi(2);
        
        if line_length_squared == 0.0 {
            return ((point[0] - line_start[0]).powi(2) + (point[1] - line_start[1]).powi(2)).sqrt();
        }
        
        let t = ((point[0] - line_start[0]) * (line_end[0] - line_start[0]) + 
                 (point[1] - line_start[1]) * (line_end[1] - line_start[1])) / line_length_squared;
        
        let t = t.clamp(0.0, 1.0);
        
        let projection = [
            line_start[0] + t * (line_end[0] - line_start[0]),
            line_start[1] + t * (line_end[1] - line_start[1])
        ];
        
        ((point[0] - projection[0]).powi(2) + (point[1] - projection[1]).powi(2)).sqrt()
    }
}
