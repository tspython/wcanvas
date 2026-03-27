use crate::app_state::State;
use crate::drawing::{DrawingElement, Element, ElementId};
use crate::state::ResizeHandle;
use crate::vector::path::Path;
use crate::vector::sdf::SdfBatch;
use crate::vector::style::StrokeStyle;
use crate::vector::tessellator::PathTessellator;
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

        #[cfg(debug_assertions)]
        {
            self.fps_sample_frames = self.fps_sample_frames.saturating_add(1);
            let elapsed = self.fps_sample_start.elapsed().as_secs_f32();
            if elapsed >= 0.25 {
                self.fps_value = self.fps_sample_frames as f32 / elapsed;
                self.fps_sample_frames = 0;
                cfg_if::cfg_if! {
                    if #[cfg(target_arch = "wasm32")] {
                        self.fps_sample_start = web_time::Instant::now();
                    } else {
                        self.fps_sample_start = std::time::Instant::now();
                    }
                }
            }
        }

        self.update_buffers();

        let (ui_vertices, ui_indices) = self.ui_renderer.generate_ui_vertices(
            self.current_tool,
            self.current_color,
            &self.color_picker,
            (self.size.width as f32, self.size.height as f32),
            self.canvas.transform.scale,
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

        if let Some(preview) = &self.input.preview_element {
            drawing_elements.push(Element {
                id: ElementId(0),
                group_id: None,
                shape: preview.clone(),
            });
        }

        if self.typing.active {
            let mut display_text = self.typing.buffer.clone();
            if self.typing.cursor_visible {
                display_text.push('|');
            }
            drawing_elements.push(Element {
                id: ElementId(0),
                group_id: None,
                shape: DrawingElement::TextBox {
                    id: self.typing.editing_id.map(|id| id.0).unwrap_or(0),
                    pos: self.typing.pos_canvas,
                    size: [
                        display_text
                            .lines()
                            .map(|line| line.chars().count())
                            .max()
                            .unwrap_or(1) as f32
                            * 19.2
                            + 16.0,
                        display_text.lines().count() as f32 * 38.0 + 16.0,
                    ],
                    content: display_text,
                    color: self.current_color,
                    font_size: 32.0,
                    state: crate::drawing::BoxState::Editing,
                },
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
        let (screen_pos, font_size) = self
            .ui_renderer
            .zoom_label_layout((self.size.width as f32, self.size.height as f32));
        self.text_renderer.add_screen_label(
            &self.gpu.device,
            &self.gpu.queue,
            &zoom_text,
            screen_pos,
            font_size,
            [1.0, 1.0, 1.0, 1.0],
        );

        #[cfg(debug_assertions)]
        {
            let fps_text = format!("{:.0} FPS", self.fps_value.max(0.0));
            let fps_size = (font_size * 2.8).clamp(36.0, 84.0);
            let estimated_width = fps_text.chars().count() as f32 * fps_size * 0.6;
            let margin = 28.0;
            let fps_pos = [
                self.size.width as f32 - estimated_width - margin,
                margin + fps_size,
            ];
            self.text_renderer.add_screen_label(
                &self.gpu.device,
                &self.gpu.queue,
                &fps_text,
                fps_pos,
                fps_size,
                [0.0, 1.0, 0.0, 1.0],
            );
        }

        self.text_renderer.build_screen_buffers(&self.gpu.device);
    }

    fn update_buffers(&mut self) {
        let mut tess = PathTessellator::new();
        let mut sdf_batch = SdfBatch::new();

        let mut all_elements = self.elements.clone();
        if let Some(preview) = &self.input.preview_element {
            all_elements.push(Element {
                id: ElementId(0),
                group_id: None,
                shape: preview.clone(),
            });
        }

        for element in all_elements.iter() {
            Self::tessellate_element(&element.shape, &mut tess, &mut sdf_batch);
        }

        if let Some(bounds) = selection_bounds(&self.elements, &self.input.selection.selected_ids) {
            Self::tessellate_selection_highlight(bounds, &mut tess);
            Self::tessellate_resize_handles(bounds, &mut tess);
            Self::tessellate_rotation_handle(bounds, &mut tess);
        }

        if let (Some(start), Some(current)) = (
            self.input.selection.marquee_start,
            self.input.selection.marquee_current,
        ) {
            Self::tessellate_marquee(start, current, &mut tess);
        }

        // Active pen stroke
        if self.input.state == crate::state::UserInputState::Drawing {
            self.tessellate_active_drawing(&mut tess);
        }

        // Upload tessellated geometry
        let (vertices, indices) = tess.finish();
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

        // Upload SDF geometry
        let (sdf_vertices, sdf_indices) = sdf_batch.finish();
        if !sdf_vertices.is_empty() {
            self.sdf_geo.vertex = Some(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("SDF Vertex Buffer"),
                    contents: bytemuck::cast_slice(&sdf_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
            self.sdf_geo.index = Some(self.gpu.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("SDF Index Buffer"),
                    contents: bytemuck::cast_slice(&sdf_indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
            self.sdf_geo.count = sdf_indices.len() as u32;
        } else {
            self.sdf_geo.vertex = None;
            self.sdf_geo.index = None;
            self.sdf_geo.count = 0;
        }
    }

    /// Convert a DrawingElement into GPU geometry.
    ///
    /// Clean shapes without rough_style are rendered via SDF for resolution-independent
    /// vector rendering. Rough-styled shapes and freehand strokes are tessellated into
    /// triangle geometry via the PathTessellator.
    fn tessellate_element(
        element: &DrawingElement,
        tess: &mut PathTessellator,
        sdf_batch: &mut SdfBatch,
    ) {
        match element {
            DrawingElement::Stroke {
                points,
                color,
                width,
            } => {
                let path = Path::from_points(points);
                tess.stroke(&path, &StrokeStyle::new(*color, *width));
            }

            DrawingElement::Rectangle {
                position,
                size,
                rotation,
                color,
                fill,
                stroke_width,
                rough_style,
            } => {
                let corners = rotated_rect_points(*position, *size, *rotation);
                if let Some(rough_options) = rough_style {
                    // Rough style: tessellate the rough path segments
                    let mut generator = crate::rough::RoughGenerator::new(rough_options.seed);
                    let rough_lines = generator.rough_rectangle(*position, *size, rough_options);
                    let style = StrokeStyle::new(*color, rough_options.stroke_width);
                    for mut line_points in rough_lines {
                        if rotation.abs() > f32::EPSILON {
                            let center = selection_center_from_bounds(*position, *size);
                            rotate_points_in_place(&mut line_points, center, *rotation);
                        }
                        let path = Path::from_points(&line_points);
                        tess.stroke(&path, &style);
                    }
                } else if rotation.abs() > f32::EPSILON {
                    if *fill {
                        tess.fill_convex(&corners, *color);
                    }
                    tess.stroke_polygon(&corners, &StrokeStyle::new(*color, *stroke_width));
                } else {
                    // Clean shape: SDF vector rendering
                    sdf_batch.add_rect(*position, *size, *color, *stroke_width, *fill);
                }
            }

            DrawingElement::Circle {
                center,
                radius,
                color,
                fill,
                stroke_width,
                rough_style,
            } => {
                if let Some(rough_options) = rough_style {
                    let mut generator = crate::rough::RoughGenerator::new(rough_options.seed);
                    let diameter = *radius * 2.0;
                    let rough_lines =
                        generator.rough_ellipse(*center, diameter, diameter, rough_options);
                    let style = StrokeStyle::new(*color, rough_options.stroke_width);
                    for line_points in rough_lines {
                        let path = Path::from_points(&line_points);
                        tess.stroke(&path, &style);
                    }
                } else {
                    // Clean shape: SDF vector rendering
                    sdf_batch.add_circle(*center, *radius, *color, *stroke_width, *fill);
                }
            }

            DrawingElement::Diamond {
                position,
                size,
                rotation,
                color,
                fill,
                stroke_width,
                rough_style,
            } => {
                let points = rotated_diamond_points(*position, *size, *rotation);
                if let Some(rough_options) = rough_style {
                    let mut generator = crate::rough::RoughGenerator::new(rough_options.seed);
                    let rough_lines = generator.rough_diamond(*position, *size, rough_options);
                    let style = StrokeStyle::new(*color, rough_options.stroke_width);
                    for mut line_points in rough_lines {
                        if rotation.abs() > f32::EPSILON {
                            let center = selection_center_from_bounds(*position, *size);
                            rotate_points_in_place(&mut line_points, center, *rotation);
                        }
                        let path = Path::from_points(&line_points);
                        tess.stroke(&path, &style);
                    }
                } else if rotation.abs() > f32::EPSILON {
                    if *fill {
                        tess.fill_convex(&points, *color);
                    }
                    tess.stroke_polygon(&points, &StrokeStyle::new(*color, *stroke_width));
                } else {
                    // Clean shape: SDF vector rendering
                    sdf_batch.add_diamond(*position, *size, *color, *stroke_width, *fill);
                }
            }

            DrawingElement::Arrow {
                start,
                end,
                color,
                width,
                rough_style,
            } => {
                if let Some(rough_options) = rough_style {
                    let mut generator = crate::rough::RoughGenerator::new(rough_options.seed);
                    let rough_lines = generator.rough_arrow(*start, *end, rough_options);
                    let style = StrokeStyle::new(*color, rough_options.stroke_width);
                    for line_points in rough_lines {
                        let path = Path::from_points(&line_points);
                        tess.stroke(&path, &style);
                    }
                } else {
                    let style = StrokeStyle::new(*color, *width);
                    let paths = Path::arrow(*start, *end, 20.0, 0.5);
                    for path in &paths {
                        tess.stroke(path, &style);
                    }
                }
            }

            DrawingElement::Line {
                start,
                end,
                color,
                width,
                rough_style,
            } => {
                if let Some(rough_options) = rough_style {
                    let mut generator = crate::rough::RoughGenerator::new(rough_options.seed);
                    let rough_line = generator.rough_line(*start, *end, rough_options);
                    let style = StrokeStyle::new(*color, rough_options.stroke_width);
                    let path = Path::from_points(&rough_line);
                    tess.stroke(&path, &style);

                    if !rough_options.disable_multi_stroke {
                        let rough_line2 = generator.rough_line(*start, *end, rough_options);
                        let path2 = Path::from_points(&rough_line2);
                        tess.stroke(&path2, &style);
                    }
                } else {
                    let path = Path::line(*start, *end);
                    tess.stroke(&path, &StrokeStyle::new(*color, *width));
                }
            }

            DrawingElement::Text { .. } | DrawingElement::TextBox { .. } => {
                // Text is handled by the text renderer
            }
        }
    }

    /// Generate selection highlight geometry using the PathTessellator.
    fn tessellate_selection_highlight(bounds: ([f32; 2], [f32; 2]), tess: &mut PathTessellator) {
        let style = StrokeStyle::new([0.0, 0.5, 1.0, 0.8], 3.0);
        let margin = 6.0;
        let min = [bounds.0[0] - margin, bounds.0[1] - margin];
        let max = [bounds.1[0] + margin, bounds.1[1] + margin];
        let path = Path::rect(min, [max[0] - min[0], max[1] - min[1]]);
        tess.stroke(&path, &style);
    }

    fn tessellate_resize_handles(bounds: ([f32; 2], [f32; 2]), tess: &mut PathTessellator) {
        for handle_pos in handle_positions(bounds).values() {
            let size = 10.0;
            let path = Path::rect(
                [handle_pos[0] - size * 0.5, handle_pos[1] - size * 0.5],
                [size, size],
            );
            tess.fill_convex(
                &[
                    [handle_pos[0] - size * 0.5, handle_pos[1] - size * 0.5],
                    [handle_pos[0] + size * 0.5, handle_pos[1] - size * 0.5],
                    [handle_pos[0] + size * 0.5, handle_pos[1] + size * 0.5],
                    [handle_pos[0] - size * 0.5, handle_pos[1] + size * 0.5],
                ],
                [1.0, 1.0, 1.0, 1.0],
            );
            tess.stroke(&path, &StrokeStyle::new([0.0, 0.5, 1.0, 1.0], 1.5));
        }
    }

    fn tessellate_rotation_handle(bounds: ([f32; 2], [f32; 2]), tess: &mut PathTessellator) {
        let top_center = [(bounds.0[0] + bounds.1[0]) * 0.5, bounds.0[1]];
        let handle_pos = rotation_handle_position(bounds);
        tess.add_line_segment(top_center, handle_pos, [0.0, 0.5, 1.0, 1.0], 1.5);

        let radius = 6.0;
        let points = circle_points(handle_pos, radius, 24);
        tess.fill_convex(&points, [1.0, 1.0, 1.0, 1.0]);
        tess.stroke_polygon(&points, &StrokeStyle::new([0.0, 0.5, 1.0, 1.0], 1.5));
    }

    fn tessellate_marquee(start: [f32; 2], current: [f32; 2], tess: &mut PathTessellator) {
        let position = [start[0].min(current[0]), start[1].min(current[1])];
        let size = [(current[0] - start[0]).abs(), (current[1] - start[1]).abs()];
        let path = Path::rect(position, size);
        tess.stroke(&path, &StrokeStyle::new([0.0, 0.5, 1.0, 0.7], 1.5));
    }

    /// Tessellate the in-progress drawing (active pen stroke or arrow preview).
    fn tessellate_active_drawing(&self, tess: &mut PathTessellator) {
        match self.current_tool {
            crate::drawing::Tool::Pen => {
                if self.input.current_stroke.len() > 1 {
                    let path = Path::from_points(&self.input.current_stroke);
                    tess.stroke(
                        &path,
                        &StrokeStyle::new(self.current_color, self.stroke_width),
                    );
                }
            }
            crate::drawing::Tool::Arrow => {
                if let Some(start) = self.input.drag_start {
                    let end = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    let style = StrokeStyle::new(self.current_color, self.stroke_width);
                    let paths = Path::arrow(start, end, 20.0, 0.5);
                    for path in &paths {
                        tess.stroke(path, &style);
                    }
                }
            }
            _ => {}
        }
    }
}

fn selection_bounds(elements: &[Element], ids: &[ElementId]) -> Option<([f32; 2], [f32; 2])> {
    let mut iter = elements.iter().filter(|element| ids.contains(&element.id));
    let first = iter.next()?;
    let (mut min, mut max) = first.bounding_box();
    for element in iter {
        let (element_min, element_max) = element.bounding_box();
        min[0] = min[0].min(element_min[0]);
        min[1] = min[1].min(element_min[1]);
        max[0] = max[0].max(element_max[0]);
        max[1] = max[1].max(element_max[1]);
    }
    Some((min, max))
}

pub fn rotation_handle_position(bounds: ([f32; 2], [f32; 2])) -> [f32; 2] {
    let center_x = (bounds.0[0] + bounds.1[0]) * 0.5;
    [center_x, bounds.0[1] - 28.0]
}

pub fn handle_positions(
    bounds: ([f32; 2], [f32; 2]),
) -> std::collections::BTreeMap<ResizeHandle, [f32; 2]> {
    let min = bounds.0;
    let max = bounds.1;
    let center_x = (min[0] + max[0]) * 0.5;
    let center_y = (min[1] + max[1]) * 0.5;
    std::collections::BTreeMap::from([
        (ResizeHandle::NorthWest, [min[0], min[1]]),
        (ResizeHandle::North, [center_x, min[1]]),
        (ResizeHandle::NorthEast, [max[0], min[1]]),
        (ResizeHandle::East, [max[0], center_y]),
        (ResizeHandle::SouthEast, [max[0], max[1]]),
        (ResizeHandle::South, [center_x, max[1]]),
        (ResizeHandle::SouthWest, [min[0], max[1]]),
        (ResizeHandle::West, [min[0], center_y]),
    ])
}

fn selection_center_from_bounds(position: [f32; 2], size: [f32; 2]) -> [f32; 2] {
    [position[0] + size[0] * 0.5, position[1] + size[1] * 0.5]
}

fn rotate_point(point: [f32; 2], center: [f32; 2], angle: f32) -> [f32; 2] {
    let sin = angle.sin();
    let cos = angle.cos();
    let dx = point[0] - center[0];
    let dy = point[1] - center[1];
    [
        center[0] + dx * cos - dy * sin,
        center[1] + dx * sin + dy * cos,
    ]
}

fn rotate_points_in_place(points: &mut [[f32; 2]], center: [f32; 2], angle: f32) {
    for point in points {
        *point = rotate_point(*point, center, angle);
    }
}

fn rotated_rect_points(position: [f32; 2], size: [f32; 2], rotation: f32) -> [[f32; 2]; 4] {
    let center = selection_center_from_bounds(position, size);
    let mut corners = [
        position,
        [position[0] + size[0], position[1]],
        [position[0] + size[0], position[1] + size[1]],
        [position[0], position[1] + size[1]],
    ];
    if rotation.abs() > f32::EPSILON {
        rotate_points_in_place(&mut corners, center, rotation);
    }
    corners
}

fn rotated_diamond_points(position: [f32; 2], size: [f32; 2], rotation: f32) -> [[f32; 2]; 4] {
    let center = selection_center_from_bounds(position, size);
    let mut points = [
        [center[0], position[1]],
        [position[0] + size[0], center[1]],
        [center[0], position[1] + size[1]],
        [position[0], center[1]],
    ];
    if rotation.abs() > f32::EPSILON {
        rotate_points_in_place(&mut points, center, rotation);
    }
    points
}

fn circle_points(center: [f32; 2], radius: f32, segments: usize) -> Vec<[f32; 2]> {
    let mut points = Vec::with_capacity(segments);
    for i in 0..segments {
        let angle = (i as f32 / segments as f32) * std::f32::consts::TAU;
        points.push([
            center[0] + angle.cos() * radius,
            center[1] + angle.sin() * radius,
        ]);
    }
    points
}
