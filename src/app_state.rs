use crate::canvas::{CanvasTransform, Uniforms};
use crate::drawing::{DrawingElement, Tool};
use crate::state::{
    Canvas, GeometryBuffers, GpuContext, InputState, TextInput, UiBuffers,
    UiScreenBuffers, UiScreenUniforms, UserInputState::Idle,
};
use crate::text_renderer::TextRenderer;
use crate::ui::UiRenderer;
use crate::vertex::Vertex;
use std::sync::Arc;
use winit::dpi::PhysicalSize;

cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use web_time::Instant;
    } else {
        use std::time::Instant;
    }
}
use wgpu::util::DeviceExt;
use winit::window::Window;

pub struct State {
    pub window: Arc<Window>,
    pub size: PhysicalSize<u32>,

    pub gpu: GpuContext,
    pub canvas: Canvas,
    pub geometry: GeometryBuffers,
    pub ui_geo: UiBuffers,
    pub input: InputState,
    pub typing: TextInput,

    pub elements: Vec<DrawingElement>,
    pub redo_stack: Vec<DrawingElement>,
    pub current_tool: Tool,
    pub current_color: [f32; 4],
    pub stroke_width: f32,

    pub ui_renderer: UiRenderer,
    pub text_renderer: TextRenderer,
    pub ui_screen: UiScreenBuffers,
}

impl State {
    pub async fn new(window: Arc<Window>) -> State {
        let mut size = window.inner_size();
        
        #[cfg(target_arch = "wasm32")]
        {
            if size.width == 0 || size.height == 0 {
                size = winit::dpi::PhysicalSize::new(1920, 1080);
            }
        }

        let instance = wgpu::Instance::new(&wgpu::InstanceDescriptor {
            #[cfg(not(target_arch = "wasm32"))]
            backends: wgpu::Backends::PRIMARY,
            #[cfg(target_arch = "wasm32")]
            backends: wgpu::Backends::GL,
            ..Default::default()
        });

        let surface = instance.create_surface(window.clone()).unwrap();

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

        surface.configure(&device, &config);

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
                    blend: None,
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState {
                topology: wgpu::PrimitiveTopology::TriangleList,
                strip_index_format: None,
                front_face: wgpu::FrontFace::Ccw,
                cull_mode: Some(wgpu::Face::Back),
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

        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../data/shaders/ui_shader.wgsl").into(),
            ),
        });

        let ui_uniform_bind_group_layout =
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
                label: Some("ui_uniform_bind_group_layout"),
            });

        let ui_render_pipeline_layout =
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("UI Render Pipeline Layout"),
                bind_group_layouts: &[&ui_uniform_bind_group_layout],
                push_constant_ranges: &[],
            });

        let ui_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("UI Render Pipeline"),
            layout: Some(&ui_render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &ui_shader,
                entry_point: Some("vs_main"),
                buffers: &[crate::vertex::UiVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &ui_shader,
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

        let surface_format = config.format;

        let gpu = GpuContext {
            surface,
            device,
            queue,
            config,
            render_pipeline,
            ui_render_pipeline,
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
            modifiers: winit::keyboard::ModifiersState::empty(),
            state: Idle,
            pan_start: None,
            current_stroke: Vec::new(),
            drag_start: None,
            selected_element: None,
            element_start_pos: None,
            preview_element: None,
        };

        let typing = TextInput {
            active: false,
            buffer: String::new(),
            pos_canvas: [0.0; 2],
            cursor_visible: false,
            blink_timer: Instant::now(),
        };

        let ui_renderer = UiRenderer::new();
        let text_renderer = TextRenderer::new(&gpu.device, &gpu.queue, surface_format, &uniform_bind_group_layout, &ui_uniform_bind_group_layout);

        let ui_screen_uniforms = UiScreenUniforms {
            screen_size: [size.width as f32, size.height as f32],
            _padding: [0.0, 0.0],
        };

        let ui_screen_uniform_buffer = gpu.device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("UI Screen Uniform Buffer"),
            contents: bytemuck::cast_slice(&[ui_screen_uniforms]),
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
        });

        let ui_screen_bind_group = gpu.device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &ui_uniform_bind_group_layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: ui_screen_uniform_buffer.as_entire_binding(),
            }],
            label: Some("ui_screen_bind_group"),
        });

        let ui_screen = UiScreenBuffers {
            uniform: ui_screen_uniform_buffer,
            bind_group: ui_screen_bind_group,
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
            redo_stack: Vec::new(),
            current_tool: Tool::Pen,
            current_color: [0.0, 0.0, 0.0, 1.0], 
            stroke_width: 2.0,
            ui_renderer,
            text_renderer,
            ui_screen,
        }
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }
} 