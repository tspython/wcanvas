use crate::canvas::{CanvasTransform, Uniforms};
use crate::document::Document;
use crate::drawing::{Element, ElementId, Tool, sync_id_counters};
use crate::history::{Action, History};
use crate::state::{
    Canvas, ColorPickerState, GeometryBuffers, GpuContext, InputState, SdfBuffers, SelectionState,
    TextInput, UiBuffers, UiScreenBuffers, UiScreenUniforms, UserInputState::Idle,
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
    pub sdf_geo: SdfBuffers,
    pub ui_geo: UiBuffers,
    pub input: InputState,
    pub typing: TextInput,

    pub elements: Vec<Element>,
    pub history: History,
    pub current_tool: Tool,
    pub current_color: [f32; 4],
    pub color_picker: ColorPickerState,
    pub stroke_width: f32,
    pub clipboard: Vec<Element>,

    pub ui_renderer: UiRenderer,
    pub text_renderer: TextRenderer,
    pub ui_screen: UiScreenBuffers,

    /// Path of the currently open file (native only).
    pub current_file_path: Option<String>,
    /// Document name for display.
    pub document_name: String,

    #[cfg(debug_assertions)]
    pub fps_sample_start: Instant,
    #[cfg(debug_assertions)]
    pub fps_sample_frames: u32,
    #[cfg(debug_assertions)]
    pub fps_value: f32,
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
                    blend: Some(wgpu::BlendState::ALPHA_BLENDING),
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

        let sdf_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("SDF Vector Shape Shader"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../data/shaders/vector_sdf_shader.wgsl").into(),
            ),
        });

        let sdf_render_pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
            label: Some("SDF Render Pipeline"),
            layout: Some(&render_pipeline_layout),
            vertex: wgpu::VertexState {
                module: &sdf_shader,
                entry_point: Some("vs_main"),
                buffers: &[crate::vertex::SdfVertex::desc()],
                compilation_options: wgpu::PipelineCompilationOptions::default(),
            },
            fragment: Some(wgpu::FragmentState {
                module: &sdf_shader,
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

        let ui_shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("UI Shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("../data/shaders/ui_shader.wgsl").into()),
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
            sdf_render_pipeline,
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

        let sdf_geo = SdfBuffers {
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
            transform_snapshot: Vec::new(),
            selection: SelectionState::new(),
            preview_element: None,
        };

        let typing = TextInput {
            active: false,
            buffer: String::new(),
            pos_canvas: [0.0; 2],
            editing_id: None,
            cursor_pos: 0,
            cursor_visible: false,
            blink_timer: Instant::now(),
        };

        let ui_renderer = UiRenderer::new();
        let text_renderer = TextRenderer::new(
            &gpu.device,
            &gpu.queue,
            surface_format,
            &uniform_bind_group_layout,
            &ui_uniform_bind_group_layout,
        );

        let ui_screen_uniforms = UiScreenUniforms {
            screen_size: [size.width as f32, size.height as f32],
            _padding: [0.0, 0.0],
        };

        let ui_screen_uniform_buffer =
            gpu.device
                .create_buffer_init(&wgpu::util::BufferInitDescriptor {
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
            sdf_geo,
            ui_geo,
            input,
            typing,
            elements: Vec::new(),
            history: History::default(),
            current_tool: Tool::Pen,
            current_color: [0.0, 0.0, 0.0, 1.0],
            color_picker: ColorPickerState::new(),
            stroke_width: 2.0,
            clipboard: Vec::new(),
            ui_renderer,
            text_renderer,
            ui_screen,
            current_file_path: None,
            document_name: "Untitled".to_string(),
            #[cfg(debug_assertions)]
            fps_sample_start: Instant::now(),
            #[cfg(debug_assertions)]
            fps_sample_frames: 0,
            #[cfg(debug_assertions)]
            fps_value: 0.0,
        }
    }

    pub fn window(&self) -> &Arc<Window> {
        &self.window
    }

    /// Save the current canvas to a Document.
    pub fn to_document(&self) -> Document {
        Document::from_state(
            &self.elements,
            self.canvas.transform.offset,
            self.canvas.transform.scale,
            Some(&self.document_name),
        )
    }

    /// Load a Document into the current state.
    pub fn load_document(&mut self, doc: Document) {
        self.elements = doc.elements;
        sync_id_counters(&self.elements);
        self.history.clear();
        self.input.selection.clear();
        self.canvas.transform.offset = doc.canvas_view.offset;
        self.canvas.transform.scale = doc.canvas_view.zoom;
        self.document_name = doc.name;

        // Update GPU uniforms for the new canvas transform
        self.canvas.uniform.update_transform(
            &self.canvas.transform,
            (self.size.width as f32, self.size.height as f32),
        );
        self.gpu.queue.write_buffer(
            &self.canvas.uniform_buffer,
            0,
            bytemuck::cast_slice(&[self.canvas.uniform]),
        );
        self.sync_picker_to_color(self.current_color);
    }

    /// Save to the current file path or show Save As dialog (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn save(&mut self) {
        let doc = self.to_document();
        let json = match doc.to_json() {
            Ok(j) => j,
            Err(e) => {
                log::error!("Failed to serialize document: {}", e);
                return;
            }
        };

        let path = if let Some(ref path) = self.current_file_path {
            path.clone()
        } else {
            match crate::platform::save_file_dialog(&format!("{}.wcanvas", self.document_name)) {
                crate::platform::FileDialogResult::Selected(p) => p,
                crate::platform::FileDialogResult::Cancelled => return,
            }
        };

        match crate::platform::save_to_file(&path, &json) {
            Ok(()) => {
                log::info!("Saved to {}", path);
                self.current_file_path = Some(path.clone());
                // Extract filename for display
                if let Some(name) = std::path::Path::new(&path)
                    .file_stem()
                    .and_then(|s| s.to_str())
                {
                    self.document_name = name.to_string();
                }
            }
            Err(e) => {
                log::error!("Failed to save: {}", e);
            }
        }
    }

    /// Show Open dialog and load file (native only).
    #[cfg(not(target_arch = "wasm32"))]
    pub fn open(&mut self) {
        let path = match crate::platform::open_file_dialog() {
            crate::platform::FileDialogResult::Selected(p) => p,
            crate::platform::FileDialogResult::Cancelled => return,
        };

        match crate::platform::load_from_file(&path) {
            Ok(json) => match Document::from_json(&json) {
                Ok(doc) => {
                    self.load_document(doc);
                    self.current_file_path = Some(path.clone());
                    if let Some(name) = std::path::Path::new(&path)
                        .file_stem()
                        .and_then(|s| s.to_str())
                    {
                        self.document_name = name.to_string();
                    }
                    log::info!("Opened {}", path);
                }
                Err(e) => {
                    log::error!("Failed to parse document: {}", e);
                }
            },
            Err(e) => {
                log::error!("Failed to read file: {}", e);
            }
        }
    }

    /// Save to localStorage (WASM only).
    #[cfg(target_arch = "wasm32")]
    pub fn save_to_storage(&self) {
        let doc = self.to_document();
        match doc.to_json() {
            Ok(json) => {
                crate::platform::save_to_local_storage("wcanvas_autosave", &json);
            }
            Err(e) => {
                log::error!("Failed to serialize for localStorage: {}", e);
            }
        }
    }

    /// Load from localStorage (WASM only).
    #[cfg(target_arch = "wasm32")]
    pub fn load_from_storage(&mut self) {
        if let Some(json) = crate::platform::load_from_local_storage("wcanvas_autosave") {
            match Document::from_json(&json) {
                Ok(doc) => {
                    self.load_document(doc);
                    log::info!("Loaded from localStorage");
                }
                Err(e) => {
                    log::warn!("Failed to parse localStorage data: {}", e);
                }
            }
        }
    }

    /// Trigger a JSON file download (WASM only).
    #[cfg(target_arch = "wasm32")]
    pub fn export_download(&self) {
        let doc = self.to_document();
        match doc.to_json() {
            Ok(json) => {
                let filename = format!("{}.wcanvas", self.document_name);
                crate::platform::trigger_download(&filename, &json);
            }
            Err(e) => {
                log::error!("Failed to serialize for export: {}", e);
            }
        }
    }

    pub fn find_index_by_id(&self, id: ElementId) -> Option<usize> {
        self.elements.iter().position(|element| element.id == id)
    }

    pub fn find_element_by_id(&self, id: ElementId) -> Option<&Element> {
        self.elements.iter().find(|element| element.id == id)
    }

    pub fn find_element_mut_by_id(&mut self, id: ElementId) -> Option<&mut Element> {
        self.elements.iter_mut().find(|element| element.id == id)
    }

    pub fn snapshot_elements(&self, ids: &[ElementId]) -> Vec<Element> {
        ids.iter()
            .filter_map(|id| self.find_element_by_id(*id).cloned())
            .collect()
    }

    pub fn set_selection(&mut self, ids: Vec<ElementId>) {
        self.input.selection.selected_ids = ids;
    }

    pub fn normalize_selection(&mut self) {
        let existing_ids: std::collections::HashSet<_> =
            self.elements.iter().map(|element| element.id).collect();
        self.input
            .selection
            .selected_ids
            .retain(|id| existing_ids.contains(id));
    }

    pub fn apply_and_record(&mut self, action: Action) {
        self.apply_action(&action, true);
        self.history.push(action);
        self.normalize_selection();
        self.autosave_if_possible();
    }

    pub fn record_action(&mut self, action: Action) {
        self.history.push(action);
        self.normalize_selection();
        self.autosave_if_possible();
    }

    pub fn undo(&mut self) {
        if let Some(action) = self.history.undo_stack.pop() {
            self.apply_action(&action, false);
            self.history.redo_stack.push(action);
            self.normalize_selection();
            self.autosave_if_possible();
        }
    }

    pub fn redo(&mut self) {
        if let Some(action) = self.history.redo_stack.pop() {
            self.apply_action(&action, true);
            self.history.undo_stack.push(action);
            self.normalize_selection();
            self.autosave_if_possible();
        }
    }

    fn apply_action(&mut self, action: &Action, forward: bool) {
        match action {
            Action::Add { elements } => {
                if forward {
                    self.insert_elements(elements.iter().cloned().collect());
                } else {
                    self.remove_ids(
                        &elements
                            .iter()
                            .map(|(_, element)| element.id)
                            .collect::<Vec<_>>(),
                    );
                }
            }
            Action::Remove { elements } => {
                if forward {
                    self.remove_ids(
                        &elements
                            .iter()
                            .map(|(_, element)| element.id)
                            .collect::<Vec<_>>(),
                    );
                } else {
                    self.insert_elements(elements.iter().cloned().collect());
                }
            }
            Action::Move { before, after } | Action::ModifyProperty { before, after } => {
                let source = if forward { after } else { before };
                for element in source {
                    if let Some(index) = self.find_index_by_id(element.id) {
                        self.elements[index] = element.clone();
                    }
                }
            }
            Action::Reorder { before, after } => {
                let order = if forward { after } else { before };
                self.reorder_by_ids(order);
            }
            Action::Batch(actions) => {
                if forward {
                    for nested in actions {
                        self.apply_action(nested, true);
                    }
                } else {
                    for nested in actions.iter().rev() {
                        self.apply_action(nested, false);
                    }
                }
            }
        }
    }

    fn insert_elements(&mut self, mut entries: Vec<(usize, Element)>) {
        entries.sort_by_key(|(index, _)| *index);
        for (index, element) in entries {
            let insert_index = index.min(self.elements.len());
            self.elements.insert(insert_index, element);
        }
    }

    fn remove_ids(&mut self, ids: &[ElementId]) {
        self.elements.retain(|element| !ids.contains(&element.id));
        self.input
            .selection
            .selected_ids
            .retain(|id| !ids.contains(id));
    }

    fn reorder_by_ids(&mut self, order: &[ElementId]) {
        let mut reordered = Vec::with_capacity(self.elements.len());
        for id in order {
            if let Some(index) = self.find_index_by_id(*id) {
                reordered.push(self.elements[index].clone());
            }
        }
        self.elements = reordered;
    }

    fn autosave_if_possible(&self) {
        #[cfg(target_arch = "wasm32")]
        {
            self.save_to_storage();
        }
        #[cfg(not(target_arch = "wasm32"))]
        {
            if let Ok(path) = crate::platform::autosave_path() {
                let doc = self.to_document();
                if let Ok(json) = doc.to_json() {
                    if let Err(e) =
                        crate::platform::save_to_file(path.to_str().unwrap_or(""), &json)
                    {
                        log::warn!("Autosave failed: {}", e);
                    }
                }
            }
        }
    }

    pub fn sync_picker_to_color(&mut self, color: [f32; 4]) {
        let (h, s, v) = rgb_to_hsv(color);
        self.color_picker.hue = h;
        self.color_picker.saturation = s;
        self.color_picker.value = v;
    }

    pub fn picker_color(&self) -> [f32; 4] {
        hsv_to_rgb(
            self.color_picker.hue,
            self.color_picker.saturation,
            self.color_picker.value,
        )
    }
}

fn rgb_to_hsv(color: [f32; 4]) -> (f32, f32, f32) {
    let r = color[0];
    let g = color[1];
    let b = color[2];
    let max = r.max(g).max(b);
    let min = r.min(g).min(b);
    let delta = max - min;

    let hue = if delta <= f32::EPSILON {
        0.0
    } else if (max - r).abs() <= f32::EPSILON {
        60.0 * ((g - b) / delta).rem_euclid(6.0)
    } else if (max - g).abs() <= f32::EPSILON {
        60.0 * (((b - r) / delta) + 2.0)
    } else {
        60.0 * (((r - g) / delta) + 4.0)
    };

    let saturation = if max <= f32::EPSILON {
        0.0
    } else {
        delta / max
    };
    (hue, saturation, max)
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
    let c = v * s;
    let hue_sector = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - ((hue_sector.rem_euclid(2.0)) - 1.0).abs());
    let (r1, g1, b1) = if hue_sector < 1.0 {
        (c, x, 0.0)
    } else if hue_sector < 2.0 {
        (x, c, 0.0)
    } else if hue_sector < 3.0 {
        (0.0, c, x)
    } else if hue_sector < 4.0 {
        (0.0, x, c)
    } else if hue_sector < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = v - c;
    [r1 + m, g1 + m, b1 + m, 1.0]
}
