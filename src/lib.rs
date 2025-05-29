mod texture;
mod ui;

use cgmath::prelude::*;
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
use ui::UiRenderer;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tool {
    Pen,
    Rectangle,
    Circle,
    Arrow,
    Text,
    Eraser,
    Select,
}

#[derive(Debug, Clone)]
enum DrawingElement {
    Stroke {
        points: Vec<[f32; 2]>,
        color: [f32; 4],
        width: f32,
    },
    Rectangle {
        position: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
        fill: bool,
        stroke_width: f32,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        fill: bool,
        stroke_width: f32,
    },
    Arrow {
        start: [f32; 2],
        end: [f32; 2],
        color: [f32; 4],
        width: f32,
    },
    Text {
        position: [f32; 2],
        content: String,
        color: [f32; 4],
        size: f32,
    },
}

struct CanvasTransform {
    offset: [f32; 2],
    scale: f32,
}

impl CanvasTransform {
    fn new() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: 1.0,
        }
    }

    fn screen_to_canvas(&self, screen_pos: [f32; 2]) -> [f32; 2] {
        [
            (screen_pos[0] - self.offset[0]) / self.scale,
            (screen_pos[1] - self.offset[1]) / self.scale,
        ]
    }

    fn canvas_to_screen(&self, canvas_pos: [f32; 2]) -> [f32; 2] {
        [
            canvas_pos[0] * self.scale + self.offset[0],
            canvas_pos[1] * self.scale + self.offset[1],
        ]
    }
}

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

#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Uniforms {
    transform: [[f32; 4]; 4],
}

impl Uniforms {
    fn new() -> Self {
        Self {
            transform: cgmath::Matrix4::identity().into(),
        }
    }

    fn update_transform(&mut self, canvas_transform: &CanvasTransform, window_size: (f32, f32)) {
        let proj = cgmath::ortho(0.0, window_size.0, window_size.1, 0.0, -1.0, 1.0);
        
        let translate = cgmath::Matrix4::from_translation(cgmath::Vector3::new(
            canvas_transform.offset[0],
            canvas_transform.offset[1],
            0.0,
        ));
        let scale = cgmath::Matrix4::from_scale(canvas_transform.scale);
        
        self.transform = (proj * translate * scale).into();
    }
}

struct State<'a> {
    surface: wgpu::Surface<'a>,
    device: wgpu::Device,
    queue: wgpu::Queue,
    config: wgpu::SurfaceConfiguration,
    size: winit::dpi::PhysicalSize<u32>,
    window: &'a Window,
    render_pipeline: wgpu::RenderPipeline,
    
    // Drawing state
    elements: Vec<DrawingElement>,
    current_tool: Tool,
    current_color: [f32; 4],
    current_stroke_width: f32,
    
    // Canvas transform
    canvas_transform: CanvasTransform,
    
    // Input state
    mouse_pos: [f32; 2],
    is_drawing: bool,
    current_stroke: Vec<[f32; 2]>,
    drag_start: Option<[f32; 2]>,
    is_panning: bool,
    pan_start: Option<([f32; 2], [f32; 2])>, // (mouse_pos, offset)
    modifiers_state: ModifiersState,
    
    // Uniforms
    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,
    
    // Dynamic vertex buffer for drawing
    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    num_indices: u32,
    
    // UI
    ui_renderer: UiRenderer,
    ui_vertex_buffer: Option<wgpu::Buffer>,
    ui_index_buffer: Option<wgpu::Buffer>,
    ui_num_indices: u32,
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

        // Create uniforms
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

        // Create shader module
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("draw_shader.wgsl").into()),
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

        Self {
            surface,
            device,
            queue,
            config,
            size,
            window,
            render_pipeline,
            elements: Vec::new(),
            current_tool: Tool::Pen,
            current_color: [0.0, 0.0, 0.0, 1.0], // Black
            current_stroke_width: 2.0,
            canvas_transform,
            mouse_pos: [0.0, 0.0],
            is_drawing: false,
            current_stroke: Vec::new(),
            drag_start: None,
            is_panning: false,
            pan_start: None,
            modifiers_state: ModifiersState::empty(),
            uniforms,
            uniform_buffer,
            uniform_bind_group,
            vertex_buffer: None,
            index_buffer: None,
            num_indices: 0,
            ui_renderer: UiRenderer::new(),
            ui_vertex_buffer: None,
            ui_index_buffer: None,
            ui_num_indices: 0,
        }
    }

    pub fn window(&self) -> &Window {
        &self.window
    }

    fn resize(&mut self, new_size: winit::dpi::PhysicalSize<u32>) {
        if new_size.width > 0 && new_size.height > 0 {
            self.size = new_size;
            self.config.width = new_size.width;
            self.config.height = new_size.height;
            self.surface.configure(&self.device, &self.config);
            
            // Update projection matrix
            self.uniforms.update_transform(
                &self.canvas_transform,
                (new_size.width as f32, new_size.height as f32),
            );
            self.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.uniforms]),
            );
        }
    }

    fn input(&mut self, event: &WindowEvent) -> bool {
        match event {
            WindowEvent::ModifiersChanged(modifiers) => {
                self.modifiers_state = modifiers.state();
                false
            }
            WindowEvent::MouseInput { state, button, .. } => {
                match button {
                    MouseButton::Left => {
                        match state {
                            ElementState::Pressed => {
                                if let Some(tool) = self.ui_renderer.handle_click(self.mouse_pos) {
                                    self.current_tool = tool;
                                    return true;
                                }
                                
                                if self.modifiers_state.shift_key() {
                                    self.is_panning = true;
                                    self.pan_start = Some((self.mouse_pos, self.canvas_transform.offset));
                                } else {
                                    self.is_drawing = true;
                                    let canvas_pos = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                                    
                                    match self.current_tool {
                                        Tool::Pen => {
                                            self.current_stroke.clear();
                                            self.current_stroke.push(canvas_pos);
                                        }
                                        Tool::Rectangle | Tool::Circle | Tool::Arrow => {
                                            self.drag_start = Some(canvas_pos);
                                        }
                                        _ => {}
                                    }
                                }
                            }
                            ElementState::Released => {
                                if self.is_panning {
                                    self.is_panning = false;
                                    self.pan_start = None;
                                } else if self.is_drawing {
                                    self.is_drawing = false;
                                    self.finish_drawing();
                                }
                            }
                        }
                        true
                    }
                    MouseButton::Middle => {
                        match state {
                            ElementState::Pressed => {
                                self.is_panning = true;
                                self.pan_start = Some((self.mouse_pos, self.canvas_transform.offset));
                            }
                            ElementState::Released => {
                                self.is_panning = false;
                                self.pan_start = None;
                            }
                        }
                        true
                    }
                    _ => false,
                }
            }
            WindowEvent::CursorMoved { position, .. } => {
                self.mouse_pos = [position.x as f32, position.y as f32];
                
                if self.is_panning {
                    if let Some((start_mouse, start_offset)) = self.pan_start {
                        self.canvas_transform.offset[0] = start_offset[0] + (self.mouse_pos[0] - start_mouse[0]);
                        self.canvas_transform.offset[1] = start_offset[1] + (self.mouse_pos[1] - start_mouse[1]);
                        
                        self.uniforms.update_transform(
                            &self.canvas_transform,
                            (self.size.width as f32, self.size.height as f32),
                        );
                        self.queue.write_buffer(
                            &self.uniform_buffer,
                            0,
                            bytemuck::cast_slice(&[self.uniforms]),
                        );
                    }
                } else if self.is_drawing && self.current_tool == Tool::Pen {
                    let canvas_pos = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                    self.current_stroke.push(canvas_pos);
                }
                true
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let zoom_factor = match delta {
                    MouseScrollDelta::LineDelta(_, y) => 1.0 + y * 0.1,
                    MouseScrollDelta::PixelDelta(pos) => 1.0 + pos.y as f32 * 0.001,
                };
                
                let mouse_canvas_before = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                self.canvas_transform.scale *= zoom_factor;
                self.canvas_transform.scale = self.canvas_transform.scale.clamp(0.1, 10.0);
                let mouse_canvas_after = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                
                self.canvas_transform.offset[0] += (mouse_canvas_after[0] - mouse_canvas_before[0]) * self.canvas_transform.scale;
                self.canvas_transform.offset[1] += (mouse_canvas_after[1] - mouse_canvas_before[1]) * self.canvas_transform.scale;
                
                self.uniforms.update_transform(
                    &self.canvas_transform,
                    (self.size.width as f32, self.size.height as f32),
                );
                self.queue.write_buffer(
                    &self.uniform_buffer,
                    0,
                    bytemuck::cast_slice(&[self.uniforms]),
                );
                
                true
            }
            WindowEvent::KeyboardInput {
                event:
                    KeyEvent {
                        state: ElementState::Pressed,
                        physical_key: PhysicalKey::Code(keycode),
                        ..
                    },
                ..
            } => {
                let is_ctrl_or_cmd = self.modifiers_state.control_key() || self.modifiers_state.super_key();
                match keycode {
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
                        self.current_tool = Tool::Eraser;
                        true
                    }
                    KeyCode::KeyC => {
                        // Clear canvas
                        self.elements.clear();
                        true
                    }
                    KeyCode::KeyZ => {
                        // Undo (simple version - remove last element)
                        if is_ctrl_or_cmd {
                            self.elements.pop();
                        }
                        true
                    }
                    KeyCode::Minus => {
                        if is_ctrl_or_cmd {
                            self.canvas_transform.scale /= 1.1;
                            self.canvas_transform.scale = self.canvas_transform.scale.clamp(0.1, 10.0);
                            self.uniforms.update_transform(
                                &self.canvas_transform,
                                (self.size.width as f32, self.size.height as f32),
                            );
                            self.queue.write_buffer(
                                &self.uniform_buffer,
                                0,
                                bytemuck::cast_slice(&[self.uniforms]),
                            );
                            true
                        } else {
                            false
                        }
                    }
                    KeyCode::Equal => {
                        if is_ctrl_or_cmd {
                            self.canvas_transform.scale *= 1.1;
                            self.canvas_transform.scale = self.canvas_transform.scale.clamp(0.1, 10.0);
                            self.uniforms.update_transform(
                                &self.canvas_transform,
                                (self.size.width as f32, self.size.height as f32),
                            );
                            self.queue.write_buffer(
                                &self.uniform_buffer,
                                0,
                                bytemuck::cast_slice(&[self.uniforms]),
                            );
                            true
                        } else {
                            false
                        }
                    }
                    _ => false,
                }
            }
            _ => false,
        }
    }

    fn finish_drawing(&mut self) {
        let element = match self.current_tool {
            Tool::Pen => {
                if self.current_stroke.len() > 1 {
                    Some(DrawingElement::Stroke {
                        points: self.current_stroke.clone(),
                        color: self.current_color,
                        width: self.current_stroke_width,
                    })
                } else {
                    None
                }
            }
            Tool::Rectangle => {
                if let Some(start) = self.drag_start {
                    let end = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                    let position = [start[0].min(end[0]), start[1].min(end[1])];
                    let size = [(end[0] - start[0]).abs(), (end[1] - start[1]).abs()];
                    
                    Some(DrawingElement::Rectangle {
                        position,
                        size,
                        color: self.current_color,
                        fill: false,
                        stroke_width: self.current_stroke_width,
                    })
                } else {
                    None
                }
            }
            Tool::Circle => {
                if let Some(start) = self.drag_start {
                    let end = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                    let radius = ((end[0] - start[0]).powi(2) + (end[1] - start[1]).powi(2)).sqrt();
                    
                    Some(DrawingElement::Circle {
                        center: start,
                        radius,
                        color: self.current_color,
                        fill: false,
                        stroke_width: self.current_stroke_width,
                    })
                } else {
                    None
                }
            }
            Tool::Arrow => {
                if let Some(start) = self.drag_start {
                    let end = self.canvas_transform.screen_to_canvas(self.mouse_pos);
                    
                    Some(DrawingElement::Arrow {
                        start,
                        end,
                        color: self.current_color,
                        width: self.current_stroke_width,
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
        
        self.current_stroke.clear();
        self.drag_start = None;
    }

    fn update(&mut self) {
        self.update_buffers();
        
        let (ui_vertices, ui_indices) = self.ui_renderer.generate_ui_vertices(self.current_tool);
        
        if !ui_vertices.is_empty() {
            self.ui_vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("UI Vertex Buffer"),
                    contents: bytemuck::cast_slice(&ui_vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
            
            self.ui_index_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("UI Index Buffer"),
                    contents: bytemuck::cast_slice(&ui_indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
            
            self.ui_num_indices = ui_indices.len() as u32;
        }
    }

    fn update_buffers(&mut self) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        for element in &self.elements {
            match element {
                DrawingElement::Stroke { points, color, width } => {
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
                                index_offset, index_offset + 1, index_offset + 2,
                                index_offset, index_offset + 2, index_offset + 3,
                            ]);
                            index_offset += 4;
                        }
                    }
                }
                DrawingElement::Rectangle { position, size, color, fill, stroke_width } => {
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
                            index_offset, index_offset + 1, index_offset + 2,
                            index_offset, index_offset + 2, index_offset + 3,
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
                                    index_offset, index_offset + 1, index_offset + 2,
                                    index_offset, index_offset + 2, index_offset + 3,
                                ]);
                                index_offset += 4;
                            }
                        }
                    }
                }
                DrawingElement::Circle { center, radius, color, fill, stroke_width } => {
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
                            let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / SEGMENTS as f32;
                            
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
                                    index_offset, index_offset + 1, index_offset + 2,
                                    index_offset, index_offset + 2, index_offset + 3,
                                ]);
                                index_offset += 4;
                            }
                        }
                    }
                }
                DrawingElement::Arrow { start, end, color, width } => {
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
                            index_offset, index_offset + 1, index_offset + 2,
                            index_offset, index_offset + 2, index_offset + 3,
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
                            position: [base_x - dir_y * head_width * 0.5, base_y + dir_x * head_width * 0.5],
                            color: *color,
                        });
                        vertices.push(Vertex {
                            position: [base_x + dir_y * head_width * 0.5, base_y - dir_x * head_width * 0.5],
                            color: *color,
                        });
                        
                        indices.extend_from_slice(&[
                            index_offset + 4, index_offset + 5, index_offset + 6,
                        ]);
                        index_offset += 7;
                    }
                }
                DrawingElement::Text { .. } => {
                    // TODO: Implement text rendering
                }
            }
        }

        if self.is_drawing {
            match self.current_tool {
                Tool::Pen => {
                    if self.current_stroke.len() > 1 {
                        for i in 0..self.current_stroke.len().saturating_sub(1) {
                            let p1 = self.current_stroke[i];
                            let p2 = self.current_stroke[i + 1];
                            
                            let dx = p2[0] - p1[0];
                            let dy = p2[1] - p1[1];
                            let len = (dx * dx + dy * dy).sqrt();
                            if len > 0.0 {
                                let nx = -dy / len * self.current_stroke_width * 0.5;
                                let ny = dx / len * self.current_stroke_width * 0.5;
                                
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
                                    index_offset, index_offset + 1, index_offset + 2,
                                    index_offset, index_offset + 2, index_offset + 3,
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

        // Update GPU buffers
        if !vertices.is_empty() {
            self.vertex_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Vertex Buffer"),
                    contents: bytemuck::cast_slice(&vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                },
            ));
            
            self.index_buffer = Some(self.device.create_buffer_init(
                &wgpu::util::BufferInitDescriptor {
                    label: Some("Index Buffer"),
                    contents: bytemuck::cast_slice(&indices),
                    usage: wgpu::BufferUsages::INDEX,
                },
            ));
            
            self.num_indices = indices.len() as u32;
        } else {
            self.vertex_buffer = None;
            self.index_buffer = None;
            self.num_indices = 0;
        }
    }

    fn render(&mut self) -> Result<(), wgpu::SurfaceError> {
        let output = self.surface.get_current_texture()?;
        let view = output
            .texture
            .create_view(&wgpu::TextureViewDescriptor::default());

        let mut encoder = self
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

            render_pass.set_pipeline(&self.render_pipeline);
            render_pass.set_bind_group(0, &self.uniform_bind_group, &[]);
            
            if let (Some(vertex_buffer), Some(index_buffer)) = (&self.vertex_buffer, &self.index_buffer) {
                render_pass.set_vertex_buffer(0, vertex_buffer.slice(..));
                render_pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.num_indices, 0, 0..1);
            }
            
            let mut ui_uniforms = Uniforms::new();
            ui_uniforms.update_transform(
                &CanvasTransform::new(),
                (self.size.width as f32, self.size.height as f32),
            );
            self.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&[ui_uniforms]),
            );
            
            if let (Some(ui_vertex_buffer), Some(ui_index_buffer)) = (&self.ui_vertex_buffer, &self.ui_index_buffer) {
                render_pass.set_vertex_buffer(0, ui_vertex_buffer.slice(..));
                render_pass.set_index_buffer(ui_index_buffer.slice(..), wgpu::IndexFormat::Uint16);
                render_pass.draw_indexed(0..self.ui_num_indices, 0, 0..1);
            }
            
            self.queue.write_buffer(
                &self.uniform_buffer,
                0,
                bytemuck::cast_slice(&[self.uniforms]),
            );
        }

        self.queue.submit(std::iter::once(encoder.finish()));
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
        .run(move |event, control_flow| {
            match event {
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
            }
        })
        .unwrap();
}
