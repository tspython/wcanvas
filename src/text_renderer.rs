use crate::drawing::DrawingElement;
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

const MSDF_SIZE: u32 = 64;          
const MSDF_RANGE: f32 = 6.0;        
const MSDF_BASE_SIZE: f32 = 64.0;   
const ATLAS_SIZE: u32 = 2048; 

pub struct TextRenderer {
    font: FontArc,
    tex: wgpu::Texture,
    view: wgpu::TextureView,
    sampler: wgpu::Sampler,
    cache: HashMap<ab_glyph::GlyphId, GlyphInfo>,
    next_x: u32,
    next_y: u32,
    row_h: u32,
    pipeline: wgpu::RenderPipeline,
    screen_pipeline: Option<wgpu::RenderPipeline>,
    bind_group: wgpu::BindGroup,
    vertices: Vec<TextVertex>,
    indices: Vec<u16>,
    vbuf: Option<wgpu::Buffer>,
    ibuf: Option<wgpu::Buffer>,
    screen_vertices: Vec<TextVertex>,
    screen_indices: Vec<u16>,
    screen_vbuf: Option<wgpu::Buffer>,
    screen_ibuf: Option<wgpu::Buffer>,
}

fn generate_msdf(bitmap: &[u8], width: u32, height: u32) -> Vec<u8> {
    let msdf_width = MSDF_SIZE;
    let msdf_height = MSDF_SIZE;
    let mut msdf = vec![0u8; (msdf_width * msdf_height * 3) as usize];
    
    let scale_x = width as f32 / msdf_width as f32;
    let scale_y = height as f32 / msdf_height as f32;
    let range = MSDF_RANGE;
    
    let mut smoothed_bitmap = vec![0.0f32; (width * height) as usize];
    for y in 0..height {
        for x in 0..width {
            let mut sum = 0.0;
            let mut count = 0;
            
            for dy in -1i32..=1 {
                for dx in -1i32..=1 {
                    let nx = (x as i32 + dx).clamp(0, width as i32 - 1) as u32;
                    let ny = (y as i32 + dy).clamp(0, height as i32 - 1) as u32;
                    
                    let weight = if dx == 0 && dy == 0 { 4.0 } 
                               else if dx.abs() + dy.abs() == 1 { 2.0 } 
                               else { 1.0 };
                    
                    sum += (bitmap[(ny * width + nx) as usize] as f32 / 255.0) * weight;
                    count += weight as i32;
                }
            }
            smoothed_bitmap[(y * width + x) as usize] = sum / count as f32;
        }
    }
    
    for y in 0..msdf_height {
        for x in 0..msdf_width {
            let src_x = (x as f32 * scale_x).clamp(0.0, (width - 1) as f32);
            let src_y = (y as f32 * scale_y).clamp(0.0, (height - 1) as f32);
            
            let x0 = src_x.floor() as u32;
            let y0 = src_y.floor() as u32;
            let x1 = (x0 + 1).min(width - 1);
            let y1 = (y0 + 1).min(height - 1);
            
            let fx = src_x - x0 as f32;
            let fy = src_y - y0 as f32;
            
            let v00 = smoothed_bitmap[(y0 * width + x0) as usize];
            let v10 = smoothed_bitmap[(y0 * width + x1) as usize];
            let v01 = smoothed_bitmap[(y1 * width + x0) as usize];
            let v11 = smoothed_bitmap[(y1 * width + x1) as usize];
            
            let v0 = v00 * (1.0 - fx) + v10 * fx;
            let v1 = v01 * (1.0 - fx) + v11 * fx;
            let center_alpha = v0 * (1.0 - fy) + v1 * fy;
            
            let is_inside = center_alpha > 0.5;
            
            let mut min_dist = range;
            let search_radius = (range * scale_x.max(scale_y)) as i32 + 2;
            
            for dy in -search_radius..=search_radius {
                for dx in -search_radius..=search_radius {
                    let check_x = src_x + dx as f32 / scale_x;
                    let check_y = src_y + dy as f32 / scale_y;
                    
                    if check_x < 0.0 || check_x >= width as f32 - 1.0 || 
                       check_y < 0.0 || check_y >= height as f32 - 1.0 {
                        continue;
                    }
                    
                    let cx0 = check_x.floor() as u32;
                    let cy0 = check_y.floor() as u32;
                    let cx1 = (cx0 + 1).min(width - 1);
                    let cy1 = (cy0 + 1).min(height - 1);
                    
                    if cx1 >= width || cy1 >= height { continue; }
                    
                    let cfx = check_x - cx0 as f32;
                    let cfy = check_y - cy0 as f32;
                    
                    let cv00 = smoothed_bitmap[(cy0 * width + cx0) as usize];
                    let cv10 = smoothed_bitmap[(cy0 * width + cx1) as usize];
                    let cv01 = smoothed_bitmap[(cy1 * width + cx0) as usize];
                    let cv11 = smoothed_bitmap[(cy1 * width + cx1) as usize];
                    
                    let cv0 = cv00 * (1.0 - cfx) + cv10 * cfx;
                    let cv1 = cv01 * (1.0 - cfx) + cv11 * cfx;
                    let check_alpha = cv0 * (1.0 - cfy) + cv1 * cfy;
                    
                    let check_inside = check_alpha > 0.5;
                    
                    if is_inside != check_inside {
                        let dist = ((dx as f32 / scale_x).powi(2) + (dy as f32 / scale_y).powi(2)).sqrt();
                        min_dist = min_dist.min(dist);
                    }
                }
            }
            
            let signed_dist = if is_inside { min_dist } else { -min_dist };
            
            let normalized = (signed_dist / range + 1.0) * 0.5;
            let base_value = normalized.clamp(0.0, 1.0);
            
            let offset = 0.03;
            let r = (base_value + offset * (1.0 - base_value.abs())).clamp(0.0, 1.0);
            let g = base_value;
            let b = (base_value - offset * (1.0 - base_value.abs())).clamp(0.0, 1.0);
            
            let idx = (y * msdf_width + x) as usize * 3;
            msdf[idx] = (r * 255.0) as u8;
            msdf[idx + 1] = (g * 255.0) as u8;
            msdf[idx + 2] = (b * 255.0) as u8;
        }
    }
    
    msdf
}

impl TextRenderer {
    pub fn new(device: &wgpu::Device, queue: &wgpu::Queue, fmt: wgpu::TextureFormat,
               canvas_bind_group_layout: &wgpu::BindGroupLayout,
               ui_screen_bind_group_layout: &wgpu::BindGroupLayout) -> Self {
        let tex = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("msdf atlas"),
            size: wgpu::Extent3d {
                width: ATLAS_SIZE,
                height: ATLAS_SIZE,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: wgpu::TextureFormat::Rgba8Unorm, 
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
            bind_group_layouts: &[canvas_bind_group_layout, &bgl],
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

        let screen_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("screen-text shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../data/shaders/screen_text_shader.wgsl").into()),
        });
        let screen_pl_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("screen-text-pl"),
            bind_group_layouts: &[ui_screen_bind_group_layout, &bgl],
            push_constant_ranges: &[],
        });
        let screen_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("screen-text-pipe"),
            layout: Some(&screen_pl_layout),
            vertex: wgpu::VertexState {
                module: &screen_shader,
                entry_point: Some("vs_main"),
                buffers: &[TextVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &screen_shader,
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
            font: FontArc::try_from_slice(include_bytes!("../data/fonts/Virgil.ttf"))
                .unwrap(),
            tex,
            view,
            sampler,
            cache: HashMap::new(),
            next_x: 0,
            next_y: 0,
            row_h: 0,
            pipeline,
            screen_pipeline: Some(screen_pipeline),
            bind_group,
            vertices: Vec::new(),
            indices: Vec::new(),
            vbuf: None,
            ibuf: None,
            screen_vertices: Vec::new(),
            screen_indices: Vec::new(),
            screen_vbuf: None,
            screen_ibuf: None,
        }
    }

    fn cache_glyph(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        gid: ab_glyph::GlyphId,
    ) -> &GlyphInfo {
        match self.cache.entry(gid) {
            Entry::Occupied(entry) => entry.into_mut(),
            Entry::Vacant(vacant) => {
                let scale = ab_glyph::PxScale::from(MSDF_BASE_SIZE);
                let mut gl = gid.with_scale(scale);
                gl.position = point(0.0, 0.0);

                let maybe_out = self.font.outline_glyph(gl.clone());

                if maybe_out.is_none() {
                    let info = GlyphInfo {
                        uv_min: [0.0, 0.0],
                        uv_max: [0.0, 0.0],
                        size: [0.0, 0.0],
                        bearing: [0.0, 0.0],
                        advance: self.font.h_advance_unscaled(gl.id) / MSDF_BASE_SIZE,
                    };
                    return vacant.insert(info);
                }

                let out = maybe_out.unwrap();
                let bb = out.px_bounds();
                let glyph_w = bb.width() as u32;
                let glyph_h = bb.height() as u32;

                let atlas_w = MSDF_SIZE;
                let atlas_h = MSDF_SIZE;

                if self.next_x + atlas_w >= ATLAS_SIZE {
                    self.next_x = 0;
                    self.next_y += self.row_h;
                    self.row_h = 0;
                }
                if self.next_y + atlas_h >= ATLAS_SIZE {
                    panic!("glyph atlas full");
                }
                self.row_h = self.row_h.max(atlas_h);

                let mut bitmap = vec![0u8; (glyph_w * glyph_h) as usize];
                out.draw(|x, y, v| {
                    bitmap[(y * glyph_w + x) as usize] = (v * 255.0) as u8;
                });

                let msdf_data = generate_msdf(&bitmap, glyph_w, glyph_h);

                let mut rgba_data = Vec::with_capacity((atlas_w * atlas_h * 4) as usize);
                for i in 0..(atlas_w * atlas_h) as usize {
                    if i < msdf_data.len() / 3 {
                        rgba_data.push(msdf_data[i * 3]);     // R
                        rgba_data.push(msdf_data[i * 3 + 1]); // G
                        rgba_data.push(msdf_data[i * 3 + 2]); // B
                        rgba_data.push(255);                  // A
                    } else {
                        rgba_data.extend_from_slice(&[0, 0, 0, 0]);
                    }
                }

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
                    &rgba_data,
                    wgpu::TexelCopyBufferLayout {
                        offset: 0,
                        bytes_per_row: Some(atlas_w * 4),
                        rows_per_image: Some(atlas_h),
                    },
                    wgpu::Extent3d {
                        width: atlas_w,
                        height: atlas_h,
                        depth_or_array_layers: 1,
                    },
                );

                let info = GlyphInfo {
                    uv_min: [self.next_x as f32 / ATLAS_SIZE as f32, self.next_y as f32 / ATLAS_SIZE as f32],
                    uv_max: [
                        (self.next_x + atlas_w) as f32 / ATLAS_SIZE as f32,
                        (self.next_y + atlas_h) as f32 / ATLAS_SIZE as f32,
                    ],
                    size: [bb.width(), bb.height()],
                    bearing: [bb.min.x, bb.min.y],
                    advance: self.font.h_advance_unscaled(gl.id) / MSDF_BASE_SIZE,
                };

                self.next_x += atlas_w + 1;

                vacant.insert(info)
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
        for e in elems {
            if let DrawingElement::Text {
                position,
                content,
                color,
                size,
            } = e
            {
                let px = *size;
                let mut pen_x = position[0];
                let scale = ab_glyph::PxScale::from(px);
                let mut prev_gid: Option<ab_glyph::GlyphId> = None;

                for ch in content.chars() {
                    let gid = self.font.glyph_id(ch);

                    if let Some(prev) = prev_gid {
                        let kern_px = self.font.as_scaled(scale).kern(prev, gid);
                        pen_x += kern_px;
                    }

                    let info = {
                        let info_ref = self.cache_glyph(device, queue, gid);
                        *info_ref
                    };

                    let adv_px = self.font.as_scaled(scale).h_advance(gid);

                    if info.size[0] == 0.0 || info.size[1] == 0.0 {
                        pen_x += adv_px;
                        prev_gid = Some(gid);
                        continue;
                    }

                    let scale_factor = px / MSDF_BASE_SIZE;
                    let scaled_size = [info.size[0] * scale_factor, info.size[1] * scale_factor];
                    let scaled_bearing = [info.bearing[0] * scale_factor, info.bearing[1] * scale_factor];

                    let x0 = pen_x + scaled_bearing[0];
                    let y0 = position[1] + scaled_bearing[1];
                    let x1 = x0 + scaled_size[0];
                    let y1 = y0 + scaled_size[1];

                    let [u0, v0] = info.uv_min;
                    let [u1, v1] = info.uv_max;

                    self.vertices.extend_from_slice(&[
                        TextVertex {
                            pos: [x0, y0],
                            uv: [u0, v0],
                            color: *color,
                        },
                        TextVertex {
                            pos: [x1, y0],
                            uv: [u1, v0],
                            color: *color,
                        },
                        TextVertex {
                            pos: [x1, y1],
                            uv: [u1, v1],
                            color: *color,
                        },
                        TextVertex {
                            pos: [x0, y1],
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

    pub fn draw(&self, encoder: &mut wgpu::CommandEncoder, view: &wgpu::TextureView, canvas_bind_group: &wgpu::BindGroup, ui_screen_bind_group: &wgpu::BindGroup) {
        if self.vbuf.is_some() || self.screen_vbuf.is_some() {
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

            if let (Some(vb), Some(ib)) = (&self.vbuf, &self.ibuf) {
                rp.set_pipeline(&self.pipeline);
                rp.set_bind_group(0, canvas_bind_group, &[]);
                rp.set_bind_group(1, &self.bind_group, &[]);
                rp.set_vertex_buffer(0, vb.slice(..));
                rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
                rp.draw_indexed(0..self.indices.len() as u32, 0, 0..1);
            }

            self.draw_screen(&mut rp, ui_screen_bind_group);
        }
    }

    pub fn clear_screen(&mut self) {
        self.screen_vertices.clear();
        self.screen_indices.clear();
    }

    pub fn add_screen_label(&mut self,
        device:&wgpu::Device,
        queue:&wgpu::Queue,
        text:&str,
        pos_screen:[f32;2],
        px:f32,
        color:[f32;4]) {

        let mut pen_x = pos_screen[0];
        let scale = ab_glyph::PxScale::from(px);
        let mut prev_gid: Option<ab_glyph::GlyphId>=None;
        let mut off: u16 = self.screen_vertices.len() as u16;

        for ch in text.chars() {
            let gid = self.font.glyph_id(ch);
            if let Some(prev) = prev_gid {
                let kern = self.font.as_scaled(scale).kern(prev, gid);
                pen_x += kern;
            }
            let info = {*self.cache_glyph(device, queue, gid)};
            let adv = self.font.as_scaled(scale).h_advance(gid);
            if info.size[0]==0.0 || info.size[1]==0.0 {pen_x += adv; prev_gid=Some(gid); continue;}
            
            let scale_factor = px / MSDF_BASE_SIZE;
            let scaled_size = [info.size[0] * scale_factor, info.size[1] * scale_factor];
            let scaled_bearing = [info.bearing[0] * scale_factor, info.bearing[1] * scale_factor];
            
            let x0 = pen_x + scaled_bearing[0];
            let y0 = pos_screen[1] + scaled_bearing[1];
            let x1 = x0 + scaled_size[0];
            let y1 = y0 + scaled_size[1];
            let [u0,v0]=info.uv_min; let [u1,v1]=info.uv_max;
            self.screen_vertices.extend_from_slice(&[
                TextVertex{pos:[x0,y0],uv:[u0,v0],color},
                TextVertex{pos:[x1,y0],uv:[u1,v0],color},
                TextVertex{pos:[x1,y1],uv:[u1,v1],color},
                TextVertex{pos:[x0,y1],uv:[u0,v1],color},
            ]);
            self.screen_indices.extend_from_slice(&[off,off+1,off+2,off,off+2,off+3]);
            off += 4;
            pen_x += adv;
            prev_gid=Some(gid);
        }
    }

    pub fn build_screen_buffers(&mut self, device: &wgpu::Device) {
        if !self.screen_vertices.is_empty() {
            self.screen_vbuf = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("screen-text vbuf"),
                contents: bytemuck::cast_slice(&self.screen_vertices),
                usage: wgpu::BufferUsages::VERTEX,
            }));
            self.screen_ibuf = Some(device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("screen-text ibuf"),
                contents: bytemuck::cast_slice(&self.screen_indices),
                usage: wgpu::BufferUsages::INDEX,
            }));
        }
    }

    pub fn draw_screen(&self, rp:&mut wgpu::RenderPass<'_>, ui_screen_bind_group:&wgpu::BindGroup) {
        if let (Some(pipe), Some(vb), Some(ib)) = (self.screen_pipeline.as_ref(), &self.screen_vbuf, &self.screen_ibuf) {
            rp.set_pipeline(pipe);
            rp.set_bind_group(0, ui_screen_bind_group, &[]);
            rp.set_bind_group(1, &self.bind_group, &[]);
            rp.set_vertex_buffer(0, vb.slice(..));
            rp.set_index_buffer(ib.slice(..), wgpu::IndexFormat::Uint16);
            rp.draw_indexed(0..self.screen_indices.len() as u32, 0, 0..1);
        }
    }
}
