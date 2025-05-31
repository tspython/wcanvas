use crate::DrawingElement;
use ab_glyph::{Font, FontArc, ScaleFont, point};
use std::collections::HashMap;
use std::collections::hash_map::Entry;
use wgpu::util::DeviceExt;

#[repr(C)]
#[derive(Clone, Copy, bytemuck::Pod, bytemuck::Zeroable)]
pub struct TextVertex {
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
pub struct GlyphInfo {
    uv_min: [f32; 2],
    uv_max: [f32; 2],
    size: [f32; 2],
    bearing: [f32; 2],
    advance: f32,
}

pub struct TextRenderer {
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
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, fmt: wgpu::TextureFormat) -> Self {
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
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../data/shaders/text_shader.wgsl").into(),
            ),
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

    pub fn prepare(
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

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView) {
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
