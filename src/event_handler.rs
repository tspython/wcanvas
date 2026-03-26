use crate::app_state::State;
use crate::drawing::{BoxState, DrawingElement, Element, ElementId, FillStyle, GroupId, Tool};
use crate::history::Action;
use crate::state::ResizeHandle;
use crate::state::UserInputState::{Dragging, Drawing, Idle, MarqueeSelecting, Panning, Resizing};
use crate::ui::{ColorInteraction, FillInteraction};
use crate::update_logic::handle_positions;
use rand::Rng;
use winit::event::*;
use winit::keyboard::KeyCode;

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
                self.handle_mouse_input(*state, *button)
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.input.mouse_pos = [position.x as f32, position.y as f32];
                self.handle_cursor_moved()
            }
            WindowEvent::MouseWheel { delta, .. } => {
                self.handle_mouse_wheel(delta);
                true
            }
            WindowEvent::KeyboardInput {
                event: key_event, ..
            } => self.handle_keyboard_input(key_event),
            WindowEvent::PinchGesture { delta, .. } => {
                self.zoom_at_mouse(1.0 + *delta as f32);
                true
            }
            WindowEvent::Ime(Ime::Commit(text)) => self.handle_ime_commit(text),
            _ => false,
        }
    }

    fn handle_mouse_input(&mut self, state: ElementState, button: MouseButton) -> bool {
        match button {
            MouseButton::Left => match state {
                ElementState::Pressed => self.handle_left_press(),
                ElementState::Released => self.handle_left_release(),
            },
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

    fn handle_left_press(&mut self) -> bool {
        if self.commit_active_text_if_needed(false) {
            return true;
        }

        if let Some(tool) = self.ui_renderer.handle_click(
            self.input.mouse_pos,
            (self.size.width as f32, self.size.height as f32),
        ) {
            self.current_tool = tool;
            return true;
        }

        match self.ui_renderer.handle_color_interaction(
            self.input.mouse_pos,
            &self.color_picker,
            (self.size.width as f32, self.size.height as f32),
        ) {
            ColorInteraction::None => {}
            ColorInteraction::Color(color) => {
                self.apply_ui_color(color);
                return true;
            }
            ColorInteraction::TogglePicker => {
                self.color_picker.open = !self.color_picker.open;
                self.color_picker.drag_mode = None;
                if self.color_picker.open {
                    self.sync_picker_to_color(self.current_color);
                }
                return true;
            }
            ColorInteraction::BeginDrag(mode, color) => {
                self.color_picker.drag_mode = Some(mode);
                self.apply_ui_color(color);
                return true;
            }
        }

        match self.ui_renderer.handle_fill_interaction(
            self.input.mouse_pos,
            self.current_tool,
            (self.size.width as f32, self.size.height as f32),
        ) {
            FillInteraction::None => {}
            FillInteraction::SelectFill(style) => {
                self.current_fill_style = style;
                // Also apply to selected elements
                self.set_fill_style_on_selection(style);
                return true;
            }
        }

        if self.ui_renderer.is_mouse_over_ui(
            self.input.mouse_pos,
            (self.size.width as f32, self.size.height as f32),
            &self.color_picker,
            self.current_tool,
        ) || self.is_mouse_in_titlebar(self.input.mouse_pos)
        {
            return true;
        }

        if self.input.modifiers.shift_key() && self.current_tool != Tool::Select {
            self.input.state = Panning;
            self.input.pan_start = Some((self.input.mouse_pos, self.canvas.transform.offset));
            return true;
        }

        let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
        match self.current_tool {
            Tool::Select => self.handle_select_press(canvas_pos),
            Tool::Pen => {
                self.input.state = Drawing;
                self.input.current_stroke.clear();
                self.input.current_stroke.push(canvas_pos);
                true
            }
            Tool::Rectangle | Tool::Circle | Tool::Diamond | Tool::Arrow | Tool::Line => {
                self.input.state = Drawing;
                self.input.drag_start = Some(canvas_pos);
                true
            }
            Tool::Text => {
                self.start_text_editing(None, canvas_pos, String::new());
                true
            }
            Tool::Eraser => {
                if let Some(hit_id) = self.find_element_id_at_position(canvas_pos) {
                    self.remove_ids_with_history(&self.collect_group_selection(hit_id));
                    return true;
                }
                false
            }
        }
    }

    fn handle_left_release(&mut self) -> bool {
        self.color_picker.drag_mode = None;
        match self.input.state {
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
                self.finish_transform(ActionKind::Move);
            }
            Resizing => {
                self.input.state = Idle;
                self.finish_transform(ActionKind::Modify);
                self.input.selection.active_handle = None;
                self.input.selection.resize_bounds = None;
            }
            MarqueeSelecting => {
                self.finish_marquee_selection();
                self.input.state = Idle;
            }
            Idle => {}
        }
        true
    }

    fn handle_cursor_moved(&mut self) -> bool {
        if let Some(drag_mode) = self.color_picker.drag_mode {
            if let Some(color) = self.ui_renderer.handle_color_drag(
                self.input.mouse_pos,
                &self.color_picker,
                drag_mode,
                (self.size.width as f32, self.size.height as f32),
            ) {
                self.apply_ui_color(color);
            }
            return true;
        }

        if self.input.state == Panning {
            if let Some((start_mouse, start_offset)) = self.input.pan_start {
                self.canvas.transform.offset[0] =
                    start_offset[0] + (self.input.mouse_pos[0] - start_mouse[0]);
                self.canvas.transform.offset[1] =
                    start_offset[1] + (self.input.mouse_pos[1] - start_mouse[1]);
                self.flush_canvas_transform();
            }
            return true;
        }

        if self.input.state == Drawing && self.current_tool == Tool::Pen {
            if self.ui_renderer.is_mouse_over_ui(
                self.input.mouse_pos,
                (self.size.width as f32, self.size.height as f32),
                &self.color_picker,
                self.current_tool,
            ) || self.is_mouse_in_titlebar(self.input.mouse_pos)
            {
                self.finish_drawing();
                self.input.state = Idle;
                return true;
            }
            let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
            self.input.current_stroke.push(canvas_pos);
            return true;
        }

        if self.input.state == Drawing {
            self.update_preview_element();
            return true;
        }

        let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
        match self.input.state {
            Dragging => {
                self.drag_selection_to(canvas_pos);
                true
            }
            Resizing => {
                self.resize_selection_to(canvas_pos);
                true
            }
            MarqueeSelecting => {
                self.input.selection.marquee_current = Some(canvas_pos);
                true
            }
            _ => true,
        }
    }

    fn handle_keyboard_input(&mut self, key_event: &KeyEvent) -> bool {
        if key_event.state != ElementState::Pressed {
            return false;
        }

        let is_ctrl_or_cmd = self.input.modifiers.control_key() || self.input.modifiers.super_key();
        let is_shift = self.input.modifiers.shift_key();
        let is_alt = self.input.modifiers.alt_key();
        let keycode = match key_event.physical_key {
            winit::keyboard::PhysicalKey::Code(code) => code,
            _ => return false,
        };

        if self.typing.active {
            return self.handle_text_input_key(key_event, keycode, is_ctrl_or_cmd);
        }

        match keycode {
            KeyCode::Escape => {
                self.input.selection.clear();
                self.current_tool = Tool::Select;
                true
            }
            KeyCode::Delete | KeyCode::Backspace => {
                if !self.input.selection.selected_ids.is_empty() {
                    let ids = self.input.selection.selected_ids.clone();
                    self.remove_ids_with_history(&ids);
                    true
                } else {
                    false
                }
            }
            KeyCode::KeyF => {
                self.cycle_fill_on_selection();
                true
            }
            KeyCode::BracketLeft => {
                if is_ctrl_or_cmd {
                    self.reorder_selection(false, true);
                } else if is_alt {
                    self.adjust_selection_stroke_width(-0.5);
                } else {
                    self.reorder_selection(false, false);
                }
                true
            }
            KeyCode::BracketRight => {
                if is_ctrl_or_cmd {
                    self.reorder_selection(true, true);
                } else if is_alt {
                    self.adjust_selection_stroke_width(0.5);
                } else {
                    self.reorder_selection(true, false);
                }
                true
            }
            KeyCode::KeyD if is_ctrl_or_cmd => {
                self.duplicate_selection();
                true
            }
            KeyCode::KeyC if is_ctrl_or_cmd => {
                self.copy_selection();
                true
            }
            KeyCode::KeyV if is_ctrl_or_cmd => {
                self.paste_selection();
                true
            }
            KeyCode::KeyG if is_ctrl_or_cmd && is_shift => {
                self.ungroup_selection();
                true
            }
            KeyCode::KeyG if is_ctrl_or_cmd => {
                self.group_selection();
                true
            }
            KeyCode::KeyY if is_ctrl_or_cmd => {
                self.redo();
                true
            }
            KeyCode::KeyZ if is_ctrl_or_cmd => {
                self.undo();
                true
            }
            KeyCode::KeyS if is_ctrl_or_cmd => {
                #[cfg(not(target_arch = "wasm32"))]
                self.save();
                #[cfg(target_arch = "wasm32")]
                self.export_download();
                true
            }
            KeyCode::KeyO if is_ctrl_or_cmd => {
                #[cfg(not(target_arch = "wasm32"))]
                self.open();
                true
            }
            KeyCode::ArrowLeft if is_alt => {
                self.align_selection(HAlign::Left);
                true
            }
            KeyCode::ArrowRight if is_alt => {
                self.align_selection(HAlign::Right);
                true
            }
            KeyCode::ArrowUp if is_alt => {
                self.align_selection(HAlign::CenterX);
                true
            }
            KeyCode::ArrowDown if is_alt => {
                self.align_selection(HAlign::CenterY);
                true
            }
            KeyCode::Minus if is_ctrl_or_cmd => {
                self.zoom_at_mouse(0.9);
                true
            }
            KeyCode::Equal if is_ctrl_or_cmd => {
                self.zoom_at_mouse(1.1);
                true
            }
            KeyCode::Digit1 => {
                self.current_tool = Tool::Select;
                true
            }
            KeyCode::Digit2 => {
                self.current_tool = Tool::Pen;
                true
            }
            KeyCode::Digit3 => {
                self.current_tool = Tool::Rectangle;
                true
            }
            KeyCode::Digit4 => {
                self.current_tool = Tool::Circle;
                true
            }
            KeyCode::Digit5 => {
                self.current_tool = Tool::Arrow;
                true
            }
            KeyCode::Digit6 => {
                self.current_tool = Tool::Text;
                true
            }
            KeyCode::Digit7 => {
                self.current_tool = Tool::Line;
                true
            }
            KeyCode::Digit8 => {
                self.current_tool = Tool::Eraser;
                true
            }
            _ => false,
        }
    }

    fn handle_text_input_key(
        &mut self,
        key_event: &KeyEvent,
        keycode: KeyCode,
        is_ctrl_or_cmd: bool,
    ) -> bool {
        match keycode {
            KeyCode::Escape => {
                self.commit_active_text_if_needed(true);
                true
            }
            KeyCode::Enter => {
                self.insert_text_at_cursor("\n");
                true
            }
            KeyCode::Backspace => {
                if self.typing.cursor_pos > 0 {
                    let remove_at = self.typing.cursor_pos - 1;
                    self.typing.buffer.remove(remove_at);
                    self.typing.cursor_pos -= 1;
                }
                true
            }
            KeyCode::Delete => {
                if self.typing.cursor_pos < self.typing.buffer.len() {
                    self.typing.buffer.remove(self.typing.cursor_pos);
                }
                true
            }
            KeyCode::ArrowLeft => {
                self.typing.cursor_pos = self.typing.cursor_pos.saturating_sub(1);
                true
            }
            KeyCode::ArrowRight => {
                self.typing.cursor_pos = (self.typing.cursor_pos + 1).min(self.typing.buffer.len());
                true
            }
            KeyCode::ArrowUp => {
                self.move_text_cursor_vertically(-1);
                true
            }
            KeyCode::ArrowDown => {
                self.move_text_cursor_vertically(1);
                true
            }
            KeyCode::KeyC if is_ctrl_or_cmd => true,
            KeyCode::KeyV if is_ctrl_or_cmd => true,
            _ => {
                if let Some(text) = &key_event.text {
                    let visible: String = text.chars().filter(|ch| !ch.is_control()).collect();
                    if !visible.is_empty() {
                        self.insert_text_at_cursor(&visible);
                        return true;
                    }
                }
                false
            }
        }
    }

    fn handle_ime_commit(&mut self, text: &str) -> bool {
        if self.typing.active {
            let visible: String = text.chars().filter(|ch| !ch.is_control()).collect();
            if !visible.is_empty() {
                self.insert_text_at_cursor(&visible);
            }
            return true;
        }
        false
    }

    fn handle_select_press(&mut self, canvas_pos: [f32; 2]) -> bool {
        if let Some(bounds) = self.selection_bounds() {
            if let Some(handle) = self.hit_resize_handle(bounds, canvas_pos) {
                self.input.state = Resizing;
                self.input.selection.active_handle = Some(handle);
                self.input.selection.drag_origin = Some(canvas_pos);
                self.input.selection.resize_bounds = Some(bounds);
                self.input.transform_snapshot =
                    self.snapshot_elements(&self.input.selection.selected_ids);
                return true;
            }
        }

        if let Some(hit_id) = self.find_element_id_at_position(canvas_pos) {
            let double_click = self.is_double_click(hit_id);
            let clicked_ids = if double_click {
                vec![hit_id]
            } else {
                self.collect_group_selection(hit_id)
            };

            if double_click && self.begin_editing_if_text(hit_id) {
                return true;
            }

            if self.input.modifiers.shift_key() {
                let mut selected = self.input.selection.selected_ids.clone();
                for id in clicked_ids {
                    if let Some(index) = selected.iter().position(|selected_id| *selected_id == id)
                    {
                        selected.remove(index);
                    } else {
                        selected.push(id);
                    }
                }
                self.set_selection(selected);
            } else {
                self.set_selection(clicked_ids);
            }

            self.input.selection.drag_origin = Some(canvas_pos);
            self.input.transform_snapshot =
                self.snapshot_elements(&self.input.selection.selected_ids);
            self.input.state = Dragging;
            self.input.selection.last_clicked = Some((hit_id, Instant::now()));
            return true;
        }

        self.input.state = MarqueeSelecting;
        self.input.selection.marquee_start = Some(canvas_pos);
        self.input.selection.marquee_current = Some(canvas_pos);
        if !self.input.modifiers.shift_key() {
            self.set_selection(Vec::new());
        }
        true
    }

    fn drag_selection_to(&mut self, canvas_pos: [f32; 2]) {
        let Some(origin) = self.input.selection.drag_origin else {
            return;
        };
        let dx = canvas_pos[0] - origin[0];
        let dy = canvas_pos[1] - origin[1];
        let (snap_dx, snap_dy) = self.snap_delta_for_selection(dx, dy);
        for id in self.input.selection.selected_ids.clone() {
            if let Some(element) = self.find_element_mut_by_id(id) {
                element.shape.translate_by(snap_dx, snap_dy);
            }
        }
        self.input.selection.drag_origin = Some([origin[0] + snap_dx, origin[1] + snap_dy]);
    }

    fn resize_selection_to(&mut self, canvas_pos: [f32; 2]) {
        let Some(handle) = self.input.selection.active_handle else {
            return;
        };
        let Some(start_bounds) = self.input.selection.resize_bounds else {
            return;
        };
        let Some(origin) = self.input.selection.drag_origin else {
            return;
        };
        let dx = canvas_pos[0] - origin[0];
        let dy = canvas_pos[1] - origin[1];
        let new_bounds = apply_resize_handle(start_bounds, handle, dx, dy);
        let snapped = self.snap_bounds(new_bounds);
        let lock_aspect = self.input.modifiers.shift_key();

        for snapshot in self.input.transform_snapshot.clone() {
            if let Some(element) = self.find_element_mut_by_id(snapshot.id) {
                *element = snapshot.clone();
                element
                    .shape
                    .resize_to_bounds(start_bounds, snapped, lock_aspect);
            }
        }
    }

    fn finish_transform(&mut self, kind: ActionKind) {
        let before = self.input.transform_snapshot.clone();
        if before.is_empty() {
            return;
        }
        let ids: Vec<_> = before.iter().map(|element| element.id).collect();
        let after = self.snapshot_elements(&ids);
        if before == after {
            self.input.transform_snapshot.clear();
            return;
        }
        let action = match kind {
            ActionKind::Move => Action::Move { before, after },
            ActionKind::Modify => Action::ModifyProperty { before, after },
        };
        self.record_action(action);
        self.input.transform_snapshot.clear();
    }

    fn finish_marquee_selection(&mut self) {
        let (Some(start), Some(current)) = (
            self.input.selection.marquee_start,
            self.input.selection.marquee_current,
        ) else {
            return;
        };

        let bounds = normalize_bounds((start, current));
        let hits: Vec<_> = self
            .elements
            .iter()
            .filter(|element| bounds_intersect(bounds, element.bounding_box()))
            .map(|element| element.id)
            .collect();

        if self.input.modifiers.shift_key() {
            let mut selected = self.input.selection.selected_ids.clone();
            for id in hits {
                if !selected.contains(&id) {
                    selected.push(id);
                }
            }
            self.set_selection(selected);
        } else {
            self.set_selection(hits);
        }

        self.input.selection.marquee_start = None;
        self.input.selection.marquee_current = None;
    }

    fn start_text_editing(
        &mut self,
        editing_id: Option<ElementId>,
        pos: [f32; 2],
        content: String,
    ) {
        self.typing.active = true;
        self.typing.editing_id = editing_id;
        self.typing.pos_canvas = pos;
        self.typing.buffer = content;
        self.typing.cursor_pos = self.typing.buffer.len();
        self.typing.cursor_visible = true;
        self.typing.blink_timer = Instant::now();
    }

    fn commit_active_text_if_needed(&mut self, force: bool) -> bool {
        if !self.typing.active {
            return false;
        }
        if !force && self.current_tool == Tool::Text {
            return false;
        }

        let content = self.typing.buffer.clone();
        let pos = self.typing.pos_canvas;
        let size = textbox_size(&content, 32.0);
        let editing_id = self.typing.editing_id;
        self.typing.active = false;
        self.typing.editing_id = None;
        self.typing.buffer.clear();
        self.typing.cursor_pos = 0;

        if content.is_empty() {
            return true;
        }

        if let Some(id) = editing_id {
            if let Some(before) = self.find_element_by_id(id).cloned() {
                if let Some(element) = self.find_element_mut_by_id(id) {
                    element.shape = DrawingElement::TextBox {
                        id: id.0,
                        pos,
                        size,
                        content,
                        color: before.shape.color(),
                        font_size: 32.0,
                        state: BoxState::Idle,
                    };
                    let after = element.clone();
                    self.record_action(Action::ModifyProperty {
                        before: vec![before],
                        after: vec![after],
                    });
                }
            }
        } else {
            let element = Element::new(DrawingElement::TextBox {
                id: 0,
                pos,
                size,
                content,
                color: self.current_color,
                font_size: 32.0,
                state: BoxState::Idle,
            });
            let mut element = element;
            if let DrawingElement::TextBox { id, .. } = &mut element.shape {
                *id = element.id.0;
            }
            let index = self.elements.len();
            self.apply_and_record(Action::Add {
                elements: vec![(index, element.clone())],
            });
            self.set_selection(vec![element.id]);
        }
        true
    }

    fn begin_editing_if_text(&mut self, id: ElementId) -> bool {
        let Some(element) = self.find_element_by_id(id).cloned() else {
            return false;
        };

        match element.shape {
            DrawingElement::TextBox { pos, content, .. } => {
                self.start_text_editing(Some(id), pos, content);
                true
            }
            DrawingElement::Text {
                position, content, ..
            } => {
                if let Some(target) = self.find_element_mut_by_id(id) {
                    target.shape = DrawingElement::TextBox {
                        id: id.0,
                        pos: position,
                        size: textbox_size(&content, 32.0),
                        content: content.clone(),
                        color: target.shape.color(),
                        font_size: 32.0,
                        state: BoxState::Editing,
                    };
                }
                self.start_text_editing(Some(id), position, content);
                true
            }
            _ => false,
        }
    }

    fn insert_text_at_cursor(&mut self, text: &str) {
        self.typing.buffer.insert_str(self.typing.cursor_pos, text);
        self.typing.cursor_pos += text.len();
        self.typing.cursor_visible = true;
        self.typing.blink_timer = Instant::now();
    }

    fn move_text_cursor_vertically(&mut self, direction: i32) {
        let cursor = self.typing.cursor_pos.min(self.typing.buffer.len());
        let before = &self.typing.buffer[..cursor];
        let current_col = before.chars().rev().take_while(|ch| *ch != '\n').count();
        let current_line = before.chars().filter(|ch| *ch == '\n').count();
        let lines: Vec<&str> = self.typing.buffer.split('\n').collect();
        let target_line = if direction < 0 {
            current_line.saturating_sub(1)
        } else {
            (current_line + 1).min(lines.len().saturating_sub(1))
        };
        let mut target_index = 0usize;
        for (line_index, line) in lines.iter().enumerate() {
            if line_index == target_line {
                target_index += current_col.min(line.chars().count());
                break;
            }
            target_index += line.len() + 1;
        }
        self.typing.cursor_pos = target_index.min(self.typing.buffer.len());
    }

    fn apply_color_to_selection(&mut self, color: [f32; 4]) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.is_empty() {
            self.current_color = color;
            self.sync_picker_to_color(color);
            return;
        }
        let before = self.snapshot_elements(&ids);
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                element.shape.set_color(color);
            }
        }
        let after = self.snapshot_elements(&ids);
        self.current_color = color;
        self.sync_picker_to_color(color);
        self.record_action(Action::ModifyProperty { before, after });
    }

    fn apply_ui_color(&mut self, color: [f32; 4]) {
        if self.current_tool == Tool::Select && !self.input.selection.selected_ids.is_empty() {
            self.apply_color_to_selection(color);
        } else {
            self.current_color = color;
            self.sync_picker_to_color(color);
        }
    }

    fn cycle_fill_on_selection(&mut self) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.is_empty() {
            // Cycle the default fill style when nothing is selected
            self.current_fill_style = self.current_fill_style.next();
            return;
        }
        let before = self.snapshot_elements(&ids);
        let mut changed = false;
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                changed |= element.shape.cycle_fill_style();
            }
        }
        if changed {
            let after = self.snapshot_elements(&ids);
            self.record_action(Action::ModifyProperty { before, after });
        }
    }

    fn set_fill_style_on_selection(&mut self, style: FillStyle) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.is_empty() {
            return;
        }
        let before = self.snapshot_elements(&ids);
        let mut changed = false;
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                changed |= element.shape.set_fill_style(style);
            }
        }
        if changed {
            let after = self.snapshot_elements(&ids);
            self.record_action(Action::ModifyProperty { before, after });
        }
    }

    fn adjust_selection_stroke_width(&mut self, delta: f32) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.is_empty() {
            self.stroke_width = (self.stroke_width + delta).max(0.5);
            return;
        }
        let before = self.snapshot_elements(&ids);
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                let width = element.shape.stroke_width() + delta;
                element.shape.set_stroke_width(width);
            }
        }
        let after = self.snapshot_elements(&ids);
        self.record_action(Action::ModifyProperty { before, after });
    }

    fn remove_ids_with_history(&mut self, ids: &[ElementId]) {
        let removed: Vec<_> = self
            .elements
            .iter()
            .enumerate()
            .filter(|(_, element)| ids.contains(&element.id))
            .map(|(index, element)| (index, element.clone()))
            .collect();
        if removed.is_empty() {
            return;
        }
        self.apply_and_record(Action::Remove { elements: removed });
    }

    fn duplicate_selection(&mut self) {
        let selected = self.snapshot_elements(&self.input.selection.selected_ids.clone());
        if selected.is_empty() {
            return;
        }
        let new_group = if selected
            .iter()
            .all(|element| element.group_id == selected[0].group_id)
        {
            selected[0].group_id.map(|_| GroupId::next())
        } else {
            None
        };
        let duplicates: Vec<_> = selected
            .into_iter()
            .enumerate()
            .map(|(offset, mut element)| {
                element.id = ElementId::next();
                element.group_id = new_group.or(element.group_id);
                element.shape.translate_by(20.0, 20.0);
                if let DrawingElement::TextBox { id, .. } = &mut element.shape {
                    *id = element.id.0;
                }
                (self.elements.len() + offset, element)
            })
            .collect();
        let ids: Vec<_> = duplicates.iter().map(|(_, element)| element.id).collect();
        self.apply_and_record(Action::Add {
            elements: duplicates.clone(),
        });
        self.set_selection(ids);
    }

    fn copy_selection(&mut self) {
        self.clipboard = self.snapshot_elements(&self.input.selection.selected_ids.clone());
    }

    fn paste_selection(&mut self) {
        if self.clipboard.is_empty() {
            return;
        }
        let group_remap = if self
            .clipboard
            .iter()
            .any(|element| element.group_id.is_some())
        {
            Some(GroupId::next())
        } else {
            None
        };
        let pasted: Vec<_> = self
            .clipboard
            .clone()
            .into_iter()
            .enumerate()
            .map(|(offset, mut element)| {
                element.id = ElementId::next();
                if element.group_id.is_some() {
                    element.group_id = group_remap;
                }
                element.shape.translate_by(24.0, 24.0);
                if let DrawingElement::TextBox { id, .. } = &mut element.shape {
                    *id = element.id.0;
                }
                (self.elements.len() + offset, element)
            })
            .collect();
        let ids: Vec<_> = pasted.iter().map(|(_, element)| element.id).collect();
        self.apply_and_record(Action::Add {
            elements: pasted.clone(),
        });
        self.set_selection(ids);
    }

    fn reorder_selection(&mut self, forward: bool, to_edge: bool) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.is_empty() {
            return;
        }
        let before: Vec<_> = self.elements.iter().map(|element| element.id).collect();
        let mut selected = Vec::new();
        let mut others = Vec::new();
        for element in self.elements.clone() {
            if ids.contains(&element.id) {
                selected.push(element);
            } else {
                others.push(element);
            }
        }

        if to_edge {
            self.elements = if forward {
                others.into_iter().chain(selected.into_iter()).collect()
            } else {
                selected.into_iter().chain(others.into_iter()).collect()
            };
        } else {
            let mut order = self.elements.clone();
            if forward {
                for id in ids.iter().rev() {
                    if let Some(index) = order.iter().position(|element| element.id == *id) {
                        if index + 1 < order.len() {
                            order.swap(index, index + 1);
                        }
                    }
                }
            } else {
                for id in &ids {
                    if let Some(index) = order.iter().position(|element| element.id == *id) {
                        if index > 0 {
                            order.swap(index, index - 1);
                        }
                    }
                }
            }
            self.elements = order;
        }

        let after: Vec<_> = self.elements.iter().map(|element| element.id).collect();
        if before != after {
            self.record_action(Action::Reorder { before, after });
        }
    }

    fn group_selection(&mut self) {
        if self.input.selection.selected_ids.len() < 2 {
            return;
        }
        let ids = self.input.selection.selected_ids.clone();
        let before = self.snapshot_elements(&ids);
        let group_id = GroupId::next();
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                element.group_id = Some(group_id);
            }
        }
        let after = self.snapshot_elements(&ids);
        self.record_action(Action::Batch(vec![Action::ModifyProperty {
            before,
            after,
        }]));
    }

    fn ungroup_selection(&mut self) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.is_empty() {
            return;
        }
        let before = self.snapshot_elements(&ids);
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                element.group_id = None;
            }
        }
        let after = self.snapshot_elements(&ids);
        self.record_action(Action::Batch(vec![Action::ModifyProperty {
            before,
            after,
        }]));
    }

    fn align_selection(&mut self, align: HAlign) {
        let ids = self.input.selection.selected_ids.clone();
        if ids.len() < 2 {
            return;
        }
        let Some(bounds) = self.selection_bounds() else {
            return;
        };
        let before = self.snapshot_elements(&ids);
        for id in &ids {
            if let Some(element) = self.find_element_mut_by_id(*id) {
                let element_bounds = element.bounding_box();
                let dx = match align {
                    HAlign::Left => bounds.0[0] - element_bounds.0[0],
                    HAlign::Right => bounds.1[0] - element_bounds.1[0],
                    HAlign::CenterX => {
                        ((bounds.0[0] + bounds.1[0]) - (element_bounds.0[0] + element_bounds.1[0]))
                            * 0.5
                    }
                    HAlign::CenterY => {
                        ((bounds.0[1] + bounds.1[1]) - (element_bounds.0[1] + element_bounds.1[1]))
                            * 0.5
                    }
                };
                let dy = match align {
                    HAlign::CenterY => {
                        ((bounds.0[1] + bounds.1[1]) - (element_bounds.0[1] + element_bounds.1[1]))
                            * 0.5
                    }
                    _ => 0.0,
                };
                element.shape.translate_by(dx, dy);
            }
        }
        let after = self.snapshot_elements(&ids);
        self.record_action(Action::ModifyProperty { before, after });
    }

    fn selection_bounds(&self) -> Option<([f32; 2], [f32; 2])> {
        let mut iter = self
            .elements
            .iter()
            .filter(|element| self.input.selection.selected_ids.contains(&element.id));
        let first = iter.next()?;
        let (mut min, mut max) = first.bounding_box();
        for element in iter {
            let bounds = element.bounding_box();
            min[0] = min[0].min(bounds.0[0]);
            min[1] = min[1].min(bounds.0[1]);
            max[0] = max[0].max(bounds.1[0]);
            max[1] = max[1].max(bounds.1[1]);
        }
        Some((min, max))
    }

    fn hit_resize_handle(
        &self,
        bounds: ([f32; 2], [f32; 2]),
        pos: [f32; 2],
    ) -> Option<ResizeHandle> {
        for (handle, handle_pos) in handle_positions(bounds) {
            if (pos[0] - handle_pos[0]).abs() <= 10.0 && (pos[1] - handle_pos[1]).abs() <= 10.0 {
                return Some(handle);
            }
        }
        None
    }

    fn find_element_id_at_position(&self, pos: [f32; 2]) -> Option<ElementId> {
        self.elements
            .iter()
            .rev()
            .find(|element| element.shape.hit_test(pos))
            .map(|element| element.id)
    }

    fn collect_group_selection(&self, id: ElementId) -> Vec<ElementId> {
        let Some(element) = self.find_element_by_id(id) else {
            return Vec::new();
        };
        if let Some(group_id) = element.group_id {
            self.elements
                .iter()
                .filter(|candidate| candidate.group_id == Some(group_id))
                .map(|candidate| candidate.id)
                .collect()
        } else {
            vec![id]
        }
    }

    fn is_double_click(&self, id: ElementId) -> bool {
        self.input
            .selection
            .last_clicked
            .map(|(last_id, instant)| last_id == id && instant.elapsed().as_millis() < 350)
            .unwrap_or(false)
    }

    fn flush_canvas_transform(&mut self) {
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

    fn handle_mouse_wheel(&mut self, delta: &MouseScrollDelta) {
        let is_trackpad_scroll =
            matches!(delta, MouseScrollDelta::PixelDelta(_)) && !self.input.modifiers.control_key();

        if is_trackpad_scroll {
            if let MouseScrollDelta::PixelDelta(pos) = delta {
                self.canvas.transform.offset[0] += pos.x as f32;
                self.canvas.transform.offset[1] += pos.y as f32;
                self.flush_canvas_transform();
            }
            return;
        }

        let zoom_factor = match delta {
            MouseScrollDelta::LineDelta(_, y) => 1.0 + y * 0.1,
            MouseScrollDelta::PixelDelta(pos) => 1.0 + pos.y as f32 * 0.005,
        };
        self.zoom_at_mouse(zoom_factor);
    }

    fn zoom_at_mouse(&mut self, zoom_factor: f32) {
        let mouse_canvas_before = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
        self.canvas.transform.scale *= zoom_factor;
        self.canvas.transform.scale = self.canvas.transform.scale.clamp(0.1, 10.0);
        let mouse_canvas_after = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
        self.canvas.transform.offset[0] +=
            (mouse_canvas_after[0] - mouse_canvas_before[0]) * self.canvas.transform.scale;
        self.canvas.transform.offset[1] +=
            (mouse_canvas_after[1] - mouse_canvas_before[1]) * self.canvas.transform.scale;
        self.flush_canvas_transform();
    }

    pub fn finish_drawing(&mut self) {
        let element = match self.current_tool {
            Tool::Pen => {
                if self.input.current_stroke.len() > 1 {
                    Some(Element::new(DrawingElement::Stroke {
                        points: self.input.current_stroke.clone(),
                        color: self.current_color,
                        width: self.stroke_width,
                    }))
                } else {
                    None
                }
            }
            Tool::Rectangle => {
                self.shape_from_drag(|position, size, rough_style| DrawingElement::Rectangle {
                    position,
                    size,
                    color: self.current_color,
                    fill_style: self.current_fill_style,
                    stroke_width: self.stroke_width,
                    rough_style: Some(rough_style),
                })
            }
            Tool::Circle => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let radius = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
                    let mut rough_options = self.random_rough_options(0.4, 0.4, 0.5, 32.0, 0.1);
                    rough_options.stroke_width = self.stroke_width;
                    Some(Element::new(DrawingElement::Circle {
                        center: start,
                        radius,
                        color: self.current_color,
                        fill_style: self.current_fill_style,
                        stroke_width: self.stroke_width,
                        rough_style: Some(rough_options),
                    }))
                } else {
                    None
                }
            }
            Tool::Arrow => self.line_like_from_drag(true),
            Tool::Line => self.line_like_from_drag(false),
            Tool::Diamond => {
                self.shape_from_drag(|position, size, rough_style| DrawingElement::Diamond {
                    position,
                    size,
                    color: self.current_color,
                    fill_style: self.current_fill_style,
                    stroke_width: self.stroke_width,
                    rough_style: Some(rough_style),
                })
            }
            _ => None,
        };

        if let Some(element) = element {
            let index = self.elements.len();
            let id = element.id;
            self.apply_and_record(Action::Add {
                elements: vec![(index, element)],
            });
            self.set_selection(vec![id]);
        }

        self.input.current_stroke.clear();
        self.input.drag_start = None;
        self.input.preview_element = None;
    }

    fn shape_from_drag<F>(&self, shape_fn: F) -> Option<Element>
    where
        F: FnOnce([f32; 2], [f32; 2], crate::rough::RoughOptions) -> DrawingElement,
    {
        let start = self.input.drag_start?;
        let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
        let position = [start[0].min(end[0]), start[1].min(end[1])];
        let size = [(end[0] - start[0]).abs(), (end[1] - start[1]).abs()];
        let mut rough_options = self.random_rough_options(0.6, 0.8, 1.0, 16.0, 0.2);
        rough_options.stroke_width = self.stroke_width;
        Some(Element::new(shape_fn(position, size, rough_options)))
    }

    fn line_like_from_drag(&self, is_arrow: bool) -> Option<Element> {
        let start = self.input.drag_start?;
        let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
        let mut rough_options = self.random_rough_options(0.5, 0.6, 0.8, 8.0, 0.1);
        rough_options.stroke_width = self.stroke_width;
        let shape = if is_arrow {
            DrawingElement::Arrow {
                start,
                end,
                color: self.current_color,
                width: self.stroke_width,
                rough_style: Some(rough_options),
            }
        } else {
            DrawingElement::Line {
                start,
                end,
                color: self.current_color,
                width: self.stroke_width,
                rough_style: Some(rough_options),
            }
        };
        Some(Element::new(shape))
    }

    fn random_rough_options(
        &self,
        roughness_base: f32,
        roughness_variation: f32,
        randomness_base: f32,
        step_count: f32,
        tightness: f32,
    ) -> crate::rough::RoughOptions {
        let mut rough_options = crate::rough::RoughOptions::default();
        let mut rng = rand::rng();
        rough_options.roughness = roughness_base + rng.random::<f32>() * roughness_variation;
        rough_options.bowing = roughness_base + rng.random::<f32>() * roughness_variation;
        rough_options.max_randomness_offset =
            randomness_base + rng.random::<f32>() * randomness_base;
        rough_options.curve_step_count =
            (step_count + (rng.random::<f32>() * step_count * 0.25)) as u32;
        rough_options.curve_tightness = rng.random::<f32>() * tightness;
        rough_options.seed = Some(rng.random::<u64>());
        rough_options
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
                        color: [
                            self.current_color[0],
                            self.current_color[1],
                            self.current_color[2],
                            0.5,
                        ],
                        fill_style: self.current_fill_style,
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
                        color: [
                            self.current_color[0],
                            self.current_color[1],
                            self.current_color[2],
                            0.5,
                        ],
                        fill_style: self.current_fill_style,
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
                        color: [
                            self.current_color[0],
                            self.current_color[1],
                            self.current_color[2],
                            0.5,
                        ],
                        width: self.stroke_width,
                        rough_style: None,
                    });
                }
            }
            Tool::Line => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    self.input.preview_element = Some(DrawingElement::Line {
                        start,
                        end,
                        color: [
                            self.current_color[0],
                            self.current_color[1],
                            self.current_color[2],
                            0.5,
                        ],
                        width: self.stroke_width,
                        rough_style: None,
                    });
                }
            }
            Tool::Diamond => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let position = [start[0].min(end[0]), start[1].min(end[1])];
                    let size = [(end[0] - start[0]).abs(), (end[1] - start[1]).abs()];
                    self.input.preview_element = Some(DrawingElement::Diamond {
                        position,
                        size,
                        color: [
                            self.current_color[0],
                            self.current_color[1],
                            self.current_color[2],
                            0.5,
                        ],
                        fill_style: self.current_fill_style,
                        stroke_width: self.stroke_width,
                        rough_style: None,
                    });
                }
            }
            _ => {
                self.input.preview_element = None;
            }
        }
    }

    fn snap_delta_for_selection(&self, dx: f32, dy: f32) -> (f32, f32) {
        let mut snapped_dx = snap_to_grid(dx);
        let mut snapped_dy = snap_to_grid(dy);
        if let Some(bounds) = self.selection_bounds() {
            let moved = (
                [bounds.0[0] + snapped_dx, bounds.0[1] + snapped_dy],
                [bounds.1[0] + snapped_dx, bounds.1[1] + snapped_dy],
            );
            let candidates = self.snap_candidates(&self.input.selection.selected_ids);
            if let Some(adjust) =
                snap_against_candidates(moved.0[0], moved.1[0], &candidates.x_edges)
            {
                snapped_dx += adjust;
            }
            if let Some(adjust) =
                snap_against_candidates(moved.0[1], moved.1[1], &candidates.y_edges)
            {
                snapped_dy += adjust;
            }
        }
        (snapped_dx, snapped_dy)
    }

    fn snap_bounds(&self, bounds: ([f32; 2], [f32; 2])) -> ([f32; 2], [f32; 2]) {
        let mut snapped = normalize_bounds(bounds);
        snapped.0[0] = snap_to_grid(snapped.0[0]);
        snapped.0[1] = snap_to_grid(snapped.0[1]);
        snapped.1[0] = snap_to_grid(snapped.1[0]);
        snapped.1[1] = snap_to_grid(snapped.1[1]);
        snapped
    }

    fn snap_candidates(&self, excluding: &[ElementId]) -> SnapCandidates {
        let mut x_edges = Vec::new();
        let mut y_edges = Vec::new();
        for element in &self.elements {
            if excluding.contains(&element.id) {
                continue;
            }
            let bounds = element.bounding_box();
            x_edges.extend([bounds.0[0], (bounds.0[0] + bounds.1[0]) * 0.5, bounds.1[0]]);
            y_edges.extend([bounds.0[1], (bounds.0[1] + bounds.1[1]) * 0.5, bounds.1[1]]);
        }
        SnapCandidates { x_edges, y_edges }
    }
}

#[derive(Clone, Copy)]
enum ActionKind {
    Move,
    Modify,
}

#[derive(Clone, Copy)]
enum HAlign {
    Left,
    Right,
    CenterX,
    CenterY,
}

struct SnapCandidates {
    x_edges: Vec<f32>,
    y_edges: Vec<f32>,
}

fn textbox_size(content: &str, font_size: f32) -> [f32; 2] {
    let width = content
        .lines()
        .map(|line| line.chars().count() as f32 * font_size * 0.6)
        .fold(font_size * 0.8, f32::max)
        + 16.0;
    let height = content.lines().count().max(1) as f32 * font_size * 1.2 + 16.0;
    [width, height]
}

fn snap_to_grid(value: f32) -> f32 {
    const GRID: f32 = 10.0;
    (value / GRID).round() * GRID
}

fn snap_against_candidates(min: f32, max: f32, candidates: &[f32]) -> Option<f32> {
    let center = (min + max) * 0.5;
    let probes = [min, center, max];
    let mut best: Option<f32> = None;
    for probe in probes {
        for candidate in candidates {
            let delta = *candidate - probe;
            if delta.abs() <= 8.0 {
                match best {
                    Some(best_delta) if best_delta.abs() <= delta.abs() => {}
                    _ => best = Some(delta),
                }
            }
        }
    }
    best
}

fn normalize_bounds(bounds: ([f32; 2], [f32; 2])) -> ([f32; 2], [f32; 2]) {
    (
        [bounds.0[0].min(bounds.1[0]), bounds.0[1].min(bounds.1[1])],
        [bounds.0[0].max(bounds.1[0]), bounds.0[1].max(bounds.1[1])],
    )
}

fn bounds_intersect(a: ([f32; 2], [f32; 2]), b: ([f32; 2], [f32; 2])) -> bool {
    a.0[0] <= b.1[0] && a.1[0] >= b.0[0] && a.0[1] <= b.1[1] && a.1[1] >= b.0[1]
}

fn apply_resize_handle(
    bounds: ([f32; 2], [f32; 2]),
    handle: ResizeHandle,
    dx: f32,
    dy: f32,
) -> ([f32; 2], [f32; 2]) {
    let mut min = bounds.0;
    let mut max = bounds.1;
    match handle {
        ResizeHandle::NorthWest => {
            min[0] += dx;
            min[1] += dy;
        }
        ResizeHandle::North => {
            min[1] += dy;
        }
        ResizeHandle::NorthEast => {
            max[0] += dx;
            min[1] += dy;
        }
        ResizeHandle::East => {
            max[0] += dx;
        }
        ResizeHandle::SouthEast => {
            max[0] += dx;
            max[1] += dy;
        }
        ResizeHandle::South => {
            max[1] += dy;
        }
        ResizeHandle::SouthWest => {
            min[0] += dx;
            max[1] += dy;
        }
        ResizeHandle::West => {
            min[0] += dx;
        }
    }
    normalize_bounds((min, max))
}
