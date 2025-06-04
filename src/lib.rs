mod canvas;
mod drawing;
mod state;
mod text_renderer;
mod texture;
mod ui;

use canvas::CanvasTransform;
use canvas::Uniforms;
use cgmath::prelude::*;
use drawing::{DrawingElement, Tool};
use state::{
    Canvas, GeometryBuffers, GpuContext, InputState, TextInput, UiBuffers,
    UserInputState::{Drawing, Idle, Panning},
};
use text_renderer::TextRenderer;
use ui::UiRenderer;

use std::time::Instant;
#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;
use wgpu::util::DeviceExt;
use winit::{
    event::*,
    event_loop::EventLoop,
    keyboard::{KeyCode, ModifiersState, PhysicalKey},
    window::Window,
    window::WindowBuilder,
};

#[repr(C)]
#[derive(Copy, Clone, Debug, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Vertex {
    position: [f32; 2],
    color: [f32; 4],
}

impl Vertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<Vertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as wgpu::BufferAddress,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

struct State<'a> {
    window: &'a Window,
    size: winit::dpi::PhysicalSize<u32>,

    gpu: GpuContext<'a>,
    canvas: Canvas,
    geometry: GeometryBuffers,
    ui_geo: UiBuffers,
    input: InputState,
    typing: TextInput,

    elements: Vec<DrawingElement>,
    current_tool: Tool,
    current_color: [f32; 4],
    stroke_width: f32,

    ui_renderer: UiRenderer,
    text_renderer: TextRenderer,
}

impl<'a> State<'a> {
    async fn new(window: &'a Window) -> State<'a> {
        let size = window.inner_size();

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window).unwrap();

        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::default(),
                compatible_surface: Some(&surface),
                force_fallback_adapter: false,
            })
            .await
            .unwrap();

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                required_features: wgpu::Features::empty(),
                required_limits: if cfg!(target_arch = "wasm32") {
                    wgpu::Limits::downlevel_webgl2_defaults()
                } else {
                    wgpu::Limits::default()
                },
                label: None,
                memory_hints: Default::default(),
                trace: wgpu::Trace::Off,
            })
            .await
            .unwrap();

        let surface_caps = surface.get_capabilities(&adapter);
        let surface_format = surface_caps
            .formats
            .iter()
            .find(|f| f.is_srgb())
            .copied()
            .unwrap_or(surface_caps.formats[0]);

        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format: surface_format,
            width: size.width,
            height: size.height,
            present_mode: surface_caps.present_modes[0],
            alpha_mode: surface_caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
        };

        let mut uniforms = Uniforms::new();
        let canvas_transform = CanvasTransform::new();
        uniforms.update_transform(&canvas_transform, (size.width as f32, size.height as f32));

        let uniform_buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("Uniform Buffer"),
            contents: bytemuck::cast_slice(&[uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let uniform_bind_group_layout =
            device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
                entries: &[wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::VERTEX,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: None,
                    },
                    count: None,
                }],
                label: Some("uniform_bind_group_layout"),
            });

        let uniform_bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform_buffer.as_entire_binding(),
            }],
            label: Some("uniform_bind_group"),
        });

        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../data/shaders/draw_shader.wgsl").into(),
            ),
        });

        let render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("Render Pipeline Layout"),
                bind_group_layouts: &[&uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[Vertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: config.format,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: None,
                polygon_mode: wgpu::PolygonMode::Fill,
                unclipped_depth: false,
                conservative: false,
            },
            depth_stencil: None,
            multisample: wgpu::MultisampleState {
                count: 1,
                mask: !0,
                alpha_to_coverage_enabled: false,
            },
            multiview: None,
            cache: None,
        });

        surface.configure(&device, &config);

        let text_renderer = TextRenderer::new(&device, &queue, config.format);

        let gpu = GpuContext {
            surface,
            device,
            queue,
            config,
            render_pipeline,
        };

        let canvas = Canvas {
            transform: canvas_transform,
            uniform: uniforms,
            uniform_buffer,
            uniform_bind_group,
        };

        let geometry = GeometryBuffers {
            vertex: None,
            index: None,
            count: 0,
        };

        let ui_geo = UiBuffers {
            vertex: None,
            index: None,
            count: 0,
        };

        let input = InputState {
            mouse_pos: [0.0; 2],
            modifiers: ModifiersState::empty(),
            state: Idle,
            pan_start: None,
            current_stroke: Vec::new(),
            drag_start: None,
            dragging_textbox: None,
        };

        let typing = TextInput {
            active: false,
            buffer: String::new(),
            pos_canvas: [0.0; 2],
            cursor_visible: true,
            blink_timer: Instant::now(),
        };

        Self {
            window,
            size,
            gpu,
            canvas,
            geometry,
            ui_geo,
            input,
            typing,
            elements: Vec::new(),
            current_tool: Tool::Pen,
            current_color: [0.0, 0.0, 0.0, 1.0],
            stroke_width: 2.0,
            ui_renderer: UiRenderer::new(),
            text_renderer,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
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
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
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
                                if let Some(tool) =
                                    self.ui_renderer.handle_click(self.input.mouse_pos)
                                {
                                    self.current_tool = tool;
                                    return true;
                                }

                                if self.input.modifiers.shift_key() {
                                    self.input.state = Panning;
                                    self.input.pan_start =
                                        Some((self.input.mouse_pos, self.canvas.transform.offset));
                                } else {
                                    self.input.state = Drawing;
                                    let canvas_pos = self
                                        .canvas
                                        .transform
                                        .screen_to_canvas(self.input.mouse_pos);

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
                                            // Start text input mode
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
                    let canvas_pos = self.canvas.transform.screen_to_canvas(self.input.mouse_pos);
                    self.input.current_stroke.push(canvas_pos);
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
                        winit::keyboard::KeyCode::Digit7 => {
                            self.current_tool = Tool::Eraser;
                            true
                        }
                        winit::keyboard::KeyCode::KeyC => {
                            self.elements.clear();
                            true
                        }
                        winit::keyboard::KeyCode::KeyZ => {
                            if is_ctrl_or_cmd {
                                self.elements.pop();
                            }
                            true
                        }
                        winit::keyboard::KeyCode::Minus => {
                            if is_ctrl_or_cmd {
                                self.canvas.transform.scale /= 1.1;
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

    fn finish_drawing(&mut self) {
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

    fn update(&mut self) {
        if self.typing.active {
            let elapsed = self.typing.blink_timer.elapsed();
            if elapsed.as_millis() > 500 {
                self.typing.cursor_visible = !self.typing.cursor_visible;
                self.typing.blink_timer = std::time::Instant::now();
            }
        }
        self.update_buffers();

        let (ui_vertices, ui_indices) = self.ui_renderer.generate_ui_vertices(self.current_tool);

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
    }

    fn update_buffers(&mut self) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        for element in &self.elements {
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

                        // Arrowhead
                        let head_len = 15.0_f32.min(len * 0.3);
                        let head_width = width * 3.0;

                        let dir_x = dx / len;
                        let dir_y = dy / len;

                        let base_x = end[0] - dir_x * head_len;
                        let base_y = end[1] - dir_y * head_len;

                        vertices.push(Vertex {
                            position: *end,
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [
                                base_x - dir_y * head_width * 0.5,
                                base_y + dir_x * head_width * 0.5,
                            ],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [
                                base_x + dir_y * head_width * 0.5,
                                base_y - dir_x * head_width * 0.5,
                            ],
                            color: *color,
                        });

                        indices.extend_from_slice(&[
                            index_offset + 4,
                            index_offset + 5,
                            index_offset + 6,
                        ]);
                        index_offset += 7;
                    }
                }
                DrawingElement::Text { .. } => {}
                DrawingElement::TextBox { .. } => {}
            }
        }

        if self.input.state == Drawing {
            match self.current_tool {
                Tool::Pen => {
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
                _ => {
                    // TODO: Implement other tools
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

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.gpu.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
            .gpu
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("Render Encoder"),
            });

        {
            let mut render_pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("Render Pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color {
                            r: 1.0,
                            g: 1.0,
                            b: 1.0,
                            a: 1.0,
                        }),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });

            render_pass.set_pipeline(&self.gpu.render_pipeline);
            render_pass.set_bind_group(0, &self.canvas.uniform_bind_group, &[]);

            if let (Some(vertex_buffer), Some(index_buffer)) =
                (&self.geometry.vertex, &self.geometry.index)
            {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.geometry.count, 0, 0..1);
            }

            let mut ui_uniforms = Uniforms::new();
            ui_uniforms.update_transform(
                &CanvasTransform::new(),
                (self.size.width as f32, self.size.height as f32),
            );
            self.gpu.queue.write_buffer(
                &self.canvas.uniform_buffer,
                0,
                bytemuck::cast_slice(&[ui_uniforms]),
            );

            if let (Some(ui_vertex_buffer), Some(ui_index_buffer)) =
                (&self.ui_geo.vertex, &self.ui_geo.index)
            {
                render_pass.set_vertex_buffer(0, ui_vertex_buffer.slice(..));
                render_pass.set_index_buffer(ui_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.ui_geo.count, 0, 0..1);
            }

            self.gpu.queue.write_buffer(
                &self.canvas.uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.canvas.uniform]),
            );
        }

        self.text_renderer.draw(&mut encoder, &view);
        self.gpu.queue.submit(std::iter::once(encoder.finish()));
        output.present();

        Ok(())
    }
}

#[cfg_attr(target_arch = "wasm32", wasm_bindgen(start))]
pub async fn run() {
    cfg_if::cfg_if! {
        if #[cfg(target_arch = "wasm32")] {
            std::panic::set_hook(Box::new(console_error_panic_hook::hook));
            console_log::init_with_level(log::Level::Info).expect("Couldn't initialize logger");
        } else {
            env_logger::init();
        }
    }

    let event_loop = EventLoop::new().unwrap();
    let window = WindowBuilder::new()
        .with_title("wcanvas")
        .build(&event_loop)
        .unwrap();

    #[cfg(target_arch = "wasm32")]
    {
        // Winit prevents sizing with CSS, so we have to set
        // the size manually when on web.
        use winit::dpi::PhysicalSize;
        let _ = window.request_inner_size(PhysicalSize::new(800, 600));

        use winit::platform::web::WindowExtWebSys;
        web_sys::window()
            .and_then(|win| win.document())
            .and_then(|doc| {
                let dst = doc.get_element_by_id("wasm-example")?;
                let canvas = web_sys::Element::from(window.canvas()?);
                dst.append_child(&canvas).ok()?;
                Some(())
            })
            .expect("Couldn't append canvas to document body.");
    }

    let mut state = State::new(&window).await;

    event_loop
        .run(move |event, control_flow| match event {
            Event::WindowEvent {
                ref event,
                window_id,
            } if window_id == state.window().id() => {
                if !state.input(event) {
                    match event {
                        WindowEvent::CloseRequested
                        | WindowEvent::KeyboardInput {
                            event:
                                KeyEvent {
                                    state: ElementState::Pressed,
                                    physical_key: PhysicalKey::Code(KeyCode::Escape),
                                    ..
                                },
                            ..
                        } => control_flow.exit(),
                        WindowEvent::Resized(physical_size) => {
                            state.resize(*physical_size);
                        }
                        WindowEvent::RedrawRequested => {
                            state.update();
                            match state.render() {
                                Ok(_) => {}
                                Err(wgpu::SurfaceError::Lost) => state.resize(state.size),
                                Err(wgpu::SurfaceError::OutOfMemory) => control_flow.exit(),
                                Err(e) => eprintln!("{:?}", e),
                            }
                        }
                        _ => {}
                    }
                }
            }
            Event::AboutToWait => {
                state.window().request_redraw();
            }
            _ => {}
        })
        .unwrap();
}
