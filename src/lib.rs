mod texture;
mod ui;

use ab_glyph::{Font, FontRef, Glyph, point};
use cgmath::prelude::*;
use ui::UiRenderer;
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

use ab_glyph::{FontArc, PxScaleFont, ScaleFont};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use std::time::Instant;

const TEXT_SHADER: &str = r#"
@group(0) @binding(0) var tex: texture_2d<f32>;
@group(0) @binding(1) var samp: sampler;

struct VSIn {
    @location(0) pos: vec2<f32>,
    @location(1) uv:  vec2<f32>,
    @location(2) col: vec4<f32>,
};

struct VSOut {
    @builtin(position) pos: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) col: vec4<f32>,
};

@vertex
fn vs_main(v: VSIn) -> VSOut {
    var o: VSOut;
    o.pos = vec4<f32>(v.pos, 0.0, 1.0);
    o.uv  = v.uv;
    o.col = v.col;
    return o;
}

@fragment
fn fs_main(inp: VSOut) -> @location(0) vec4<f32> {
    let a = textureSample(tex, samp, inp.uv).r;
    return vec4<f32>(inp.col.rgb, inp.col.a * a);
}
"#;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
struct TextVertex {
    pos: [f32; 2],
    uv: [f32; 2],
    color: [f32; 4],
}
impl TextVertex {
    fn desc() -> wgpu::VertexBufferLayout<'static> {
        use std::mem;
        wgpu::VertexBufferLayout {
            array_stride: mem::size_of::<TextVertex>() as wgpu::BufferAddress,
            step_mode: wgpu::VertexStepMode::Vertex,
            attributes: &[
                wgpu::VertexAttribute {
                    offset: 0,
                    shader_location: 0,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 2]>() as _,
                    shader_location: 1,
                    format: wgpu::VertexFormat::Float32x2,
                },
                wgpu::VertexAttribute {
                    offset: mem::size_of::<[f32; 4]>() as _,
                    shader_location: 2,
                    format: wgpu::VertexFormat::Float32x4,
                },
            ],
        }
    }
}

#[derive(Clone, Copy)]
struct GlyphInfo {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    size: [f32; 2],
    bearing: [f32; 2],
    advance: f32,
}

struct TextRenderer {
    font: FontArc,
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    cache: HashMap<(ab_glyph::GlyphId, u32), GlyphInfo>,
    next_x: u32,
    next_y: u32,
    row_h: u32,
    pipeline: wgpu::RenderPipeline,
    bind_group: wgpu::BindGroup,
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
    vbuf: Option<wgpu::Buffer>,
    ibuf: Option<wgpu::Buffer>,
}

impl TextRenderer {
    fn new(device: &wgpu::Device, queue: &wgpu::Queue, fmt: wgpu::TextureFormat) -> Self {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glyph atlas"),
            size: wgpu::Extent3d {
                width: 1024,
                height: 1024,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::R8Unorm,
            usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = tex.create_view(&wgpu::TextureViewDescriptor::default());
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            ..Default::default()
        });
        let bgl = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("text‑bgl"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Texture {
                        multisampled: false,
                        view_dimension: wgpu::TextureViewDimension::D2,
                        sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    },
                    count: None,
                },
                wgpu::BindGroupLayoutEntry {
                    binding: 1,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            layout: &bgl,
            label: Some("text‑bg"),
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: wgpu::BindingResource::TextureView(&view),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::Sampler(&sampler),
                },
            ],
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("text‑shader"),
            source: wgpu::ShaderSource::Wgsl(TEXT_SHADER.into()),
        });
        let pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("text‑pl"),
            bind_group_layouts: &[&bgl],
            push_constant_ranges: &[],
        });
        let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("text‑pipe"),
            layout: Some(&pl_layout),
            vertex: wgpu::VertexState {
                module: &shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &shader,
                entry_point: Some("fs_main"),
                targets: &[Some(wgpu::ColorTargetState {
                    format: fmt,
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
                    write_mask: wgpu::ColorWrites::ALL,
                })],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            }),
            primitive: wgpu::PrimitiveState::default(),
            depth_stencil: None,
            multisample: wgpu::MultisampleState::default(),
            multiview: None,
            cache: None,
        });

        Self {
            font: FontArc::try_from_slice(include_bytes!("../data/fonts/SF-Pro-Text-Bold.otf"))
                .unwrap(),
            tex,
            view,
            sampler,
            cache: HashMap::new(),
            next_x: 0,
            next_y: 0,
            row_h: 0,
            pipeline,
            bind_group,
            vertices: Vec::new(),
            indices: Vec::new(),
            vbuf: None,
            ibuf: None,
        }
    }

    fn cache_glyph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gid: ab_glyph::GlyphId,
        px: u32,
    ) -> &GlyphInfo {
        let key = (gid, px);

        match self.cache.entry(key) {
            Entry::Occupied(entry) => &*entry.into_mut(),
            Entry::Vacant(vacant) => {
                let scale = ab_glyph::PxScale::from(px as f32);
                let mut gl = gid.with_scale(scale);
                gl.position = point(0.0, 0.0);

                let maybe_out = self.font.outline_glyph(gl.clone());

                if maybe_out.is_none() {
                    let info = GlyphInfo {
                        uv_min: [0.0, 0.0],
                        uv_max: [0.0, 0.0],
                        size: [0.0, 0.0],
                        bearing: [0.0, 0.0],
                        advance: self.font.h_advance_unscaled(gl.id),
                    };
                    return &*vacant.insert(info);
                }

                let out = maybe_out.unwrap();
                let bb = out.px_bounds();
                let w = bb.width() as u32;
                let h = bb.height() as u32;

                // Bump‑allocate the atlas position
                if self.next_x + w >= 1024 {
                    self.next_x = 0;
                    self.next_y += self.row_h;
                    self.row_h = 0;
                }
                if self.next_y + h >= 1024 {
                    panic!("glyph atlas full");
                }
                self.row_h = self.row_h.max(h);

                let mut buf = vec![0u8; (w * h) as usize];
                out.draw(|x, y, v| {
                    buf[(y * w + x) as usize] = (v * 255.0) as u8;
                });

                queue.write_texture(
                    wgpu::TexelCopyTextureInfo {
                        texture: &self.tex,
                        mip_level: 0,
                        origin: wgpu::Origin3d {
                            x: self.next_x,
                            y: self.next_y,
                            z: 0,
                        },
                        aspect: wgpu::TextureAspect::All,
                    },
                    &buf,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(w),
                        rows_per_image: Some(h),
                    },
                    wgpu::Extent3d {
                        width: w,
                        height: h,
                        depth_or_array_layers: 1,
                    },
                );

                let info = GlyphInfo {
                    uv_min: [self.next_x as f32 / 1024.0, self.next_y as f32 / 1024.0],
                    uv_max: [
                        (self.next_x + w) as f32 / 1024.0,
                        (self.next_y + h) as f32 / 1024.0,
                    ],
                    size: [w as f32, h as f32],
                    bearing: [bb.min.x, bb.min.y], // top‑left bearing
                    advance: self.font.h_advance_unscaled(gl.id),
                };

                self.next_x += w + 1;

                &*vacant.insert(info)
            }
        }
    }

    fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        elems: &[DrawingElement],
        viewport: (f32, f32),
    ) {
        self.vertices.clear();
        self.indices.clear();
        let mut off: u16 = 0;
        let vw = viewport.0.max(1.0);
        let vh = viewport.1.max(1.0);
        for e in elems {
            if let DrawingElement::Text {
                position,
                content,
                color,
                size,
            } = e
            {
                let px = *size as u32;
                let mut pen_x = position[0];
                let scale = ab_glyph::PxScale::from(px as f32);
                let mut prev_gid: Option<ab_glyph::GlyphId> = None;

                for ch in content.chars() {
                    let gid = self.font.glyph_id(ch);

                    if let Some(prev) = prev_gid {
                        let kern_px = self.font.as_scaled(scale).kern(prev, gid);
                        pen_x += kern_px;
                    }

                    let info = {
                        let info_ref = self.cache_glyph(device, queue, gid, px);
                        *info_ref
                    };

                    let adv_px = self.font.as_scaled(scale).h_advance(gid);

                    if info.size[0] == 0.0 || info.size[1] == 0.0 {
                        pen_x += adv_px;
                        prev_gid = Some(gid);
                        continue;
                    }

                    let x0 = pen_x + info.bearing[0];
                    let y0 = position[1] + info.bearing[1];
                    let x1 = x0 + info.size[0];
                    let y1 = y0 + info.size[1];

                    let [u0, v0] = info.uv_min;
                    let [u1, v1] = info.uv_max;

                    let nx0 = x0 / vw * 2.0 - 1.0;
                    let ny0 = 1.0 - y0 / vh * 2.0;
                    let nx1 = x1 / vw * 2.0 - 1.0;
                    let ny1 = 1.0 - y1 / vh * 2.0;

                    self.vertices.extend_from_slice(&[
                        TextVertex {
                            pos: [nx0, ny0],
                            uv: [u0, v0],
                            color: *color,
                        },
                        TextVertex {
                            pos: [nx1, ny0],
                            uv: [u1, v0],
                            color: *color,
                        },
                        TextVertex {
                            pos: [nx1, ny1],
                            uv: [u1, v1],
                            color: *color,
                        },
                        TextVertex {
                            pos: [nx0, ny1],
                            uv: [u0, v1],
                            color: *color,
                        },
                    ]);
                    self.indices
                        .extend_from_slice(&[off, off + 1, off + 2, off, off + 2, off + 3]);
                    off += 4;

                    pen_x += adv_px;
                    prev_gid = Some(gid);
                }
            }
        }
        if !self.vertices.is_empty() {
            self.vbuf = Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("text vbuf"),
                    contents: bytemuck::cast_slice(&self.vertices),
                    usage: wgpu::BufferUsages::VERTEX,
                }),
            );
            self.ibuf = Some(
                device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                    label: Some("text ibuf"),
                    contents: bytemuck::cast_slice(&self.indices),
                    usage: wgpu::BufferUsages::INDEX,
                }),
            );
        } else {
            self.vbuf = None;
            self.ibuf = None;
        }
    }

    fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
        if let (Some(vb), Some(ib)) = (&self.vbuf, &self.ibuf) {
            let mut rp = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("text pass"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Load,
                        store: wgpu::StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: None,
                occlusion_query_set: None,
                timestamp_writes: None,
            });
            rp.set_pipeline(&self.pipeline);
            rp.set_bind_group(0, &self.bind_group, &[]);
            rp.set_vertex_buffer(0, vb.slice(..));
            rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..self.indices.len() as u32, 0, 0..1);
        }
    }
}

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

    elements: Vec<DrawingElement>,
    current_tool: Tool,
    current_color: [f32; 4],
    current_stroke_width: f32,

    canvas_transform: CanvasTransform,

    mouse_pos: [f32; 2],
    is_drawing: bool,
    current_stroke: Vec<[f32; 2]>,
    drag_start: Option<[f32; 2]>,
    is_panning: bool,
    pan_start: Option<([f32; 2], [f32; 2])>,
    modifiers_state: ModifiersState,

    uniforms: Uniforms,
    uniform_buffer: wgpu::Buffer,
    uniform_bind_group: wgpu::BindGroup,

    vertex_buffer: Option<wgpu::Buffer>,
    index_buffer: Option<wgpu::Buffer>,
    num_indices: u32,

    ui_renderer: UiRenderer,
    ui_vertex_buffer: Option<wgpu::Buffer>,
    ui_index_buffer: Option<wgpu::Buffer>,
    ui_num_indices: u32,

    text_renderer: TextRenderer,

    is_typing: bool,
    text_input_buffer: String,
    text_input_position: [f32; 2],
    cursor_visible: bool,
    cursor_blink_timer: std::time::Instant,
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

        let mut text_renderer = TextRenderer::new(&device, &queue, config.format);

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
            text_renderer,
            is_typing: false,
            text_input_buffer: String::new(),
            text_input_position: [0.0, 0.0],
            cursor_visible: true,
            cursor_blink_timer: std::time::Instant::now(),
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
                                    self.pan_start =
                                        Some((self.mouse_pos, self.canvas_transform.offset));
                                } else {
                                    self.is_drawing = true;
                                    let canvas_pos =
                                        self.canvas_transform.screen_to_canvas(self.mouse_pos);

                                    match self.current_tool {
                                        Tool::Pen => {
                                            self.current_stroke.clear();
                                            self.current_stroke.push(canvas_pos);
                                        }
                                        Tool::Rectangle | Tool::Circle | Tool::Arrow => {
                                            self.drag_start = Some(canvas_pos);
                                        }
                                        Tool::Text => {
                                            let canvas_pos = self
                                                .canvas_transform
                                                .screen_to_canvas(self.mouse_pos);
                                            // Start text input mode
                                            self.is_typing = true;
                                            self.text_input_position = canvas_pos;
                                            self.text_input_buffer.clear();
                                            self.cursor_visible = true;
                                            self.cursor_blink_timer = std::time::Instant::now();
                                            return true;
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
                                self.pan_start =
                                    Some((self.mouse_pos, self.canvas_transform.offset));
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
                        self.canvas_transform.offset[0] =
                            start_offset[0] + (self.mouse_pos[0] - start_mouse[0]);
                        self.canvas_transform.offset[1] =
                            start_offset[1] + (self.mouse_pos[1] - start_mouse[1]);

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

                self.canvas_transform.offset[0] +=
                    (mouse_canvas_after[0] - mouse_canvas_before[0]) * self.canvas_transform.scale;
                self.canvas_transform.offset[1] +=
                    (mouse_canvas_after[1] - mouse_canvas_before[1]) * self.canvas_transform.scale;

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
                event: key_event, ..
            } => {
                if key_event.state != ElementState::Pressed {
                    return false;
                }

                if self.is_typing {
                    if let Some(txt) = &key_event.text {
                        for ch in txt.chars() {
                            if !ch.is_control() {
                                self.text_input_buffer.push(ch);
                                self.cursor_visible = true;
                                self.cursor_blink_timer = std::time::Instant::now();
                            }
                        }
                        if !txt.is_empty() {
                            return true;
                        }
                    }
                }

                let is_ctrl_or_cmd =
                    self.modifiers_state.control_key() || self.modifiers_state.super_key();

                let keycode_opt = match key_event.physical_key {
                    winit::keyboard::PhysicalKey::Code(code) => Some(code),
                    _ => None,
                };

                if let Some(keycode) = keycode_opt {
                    match keycode {
                        winit::keyboard::KeyCode::Backspace => {
                            if self.is_typing && !self.text_input_buffer.is_empty() {
                                self.text_input_buffer.pop();
                                return true;
                            }
                            false
                        }
                        winit::keyboard::KeyCode::Enter => {
                            if self.is_typing {
                                if !self.text_input_buffer.is_empty() {
                                    self.elements.push(DrawingElement::Text {
                                        position: self.text_input_position,
                                        content: self.text_input_buffer.clone(),
                                        color: self.current_color,
                                        size: 32.0,
                                    });
                                }
                                self.is_typing = false;
                                self.text_input_buffer.clear();
                                return true;
                            }
                            false
                        }
                        // --- existing tool shortcuts ---
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
                                self.canvas_transform.scale /= 1.1;
                                self.canvas_transform.scale =
                                    self.canvas_transform.scale.clamp(0.1, 10.0);
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
                        winit::keyboard::KeyCode::Equal => {
                            if is_ctrl_or_cmd {
                                self.canvas_transform.scale *= 1.1;
                                self.canvas_transform.scale =
                                    self.canvas_transform.scale.clamp(0.1, 10.0);
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
                } else {
                    false
                }
            }
            WindowEvent::Ime(ime) => {
                if let winit::event::Ime::Commit(text) = ime {
                    if self.is_typing {
                        for ch in text.chars() {
                            if !ch.is_control() {
                                self.text_input_buffer.push(ch);
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
        // Update cursor blink
        if self.is_typing {
            let elapsed = self.cursor_blink_timer.elapsed();
            if elapsed.as_millis() > 500 {
                self.cursor_visible = !self.cursor_visible;
                self.cursor_blink_timer = std::time::Instant::now();
            }
        }
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

        let mut drawing_elements = self.elements.clone();
        if self.is_typing {
            let mut display_text = self.text_input_buffer.clone();
            if self.cursor_visible {
                display_text.push('|');
            }
            drawing_elements.push(DrawingElement::Text {
                position: self.text_input_position,
                content: display_text,
                color: self.current_color,
                size: 32.0,
            });
        }
        self.text_renderer.prepare(
            &self.device,
            &self.queue,
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

            if let (Some(vertex_buffer), Some(index_buffer)) =
                (&self.vertex_buffer, &self.index_buffer)
            {
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

            if let (Some(ui_vertex_buffer), Some(ui_index_buffer)) =
                (&self.ui_vertex_buffer, &self.ui_index_buffer)
            {
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

        self.text_renderer.draw(&mut encoder, &view);
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
