cfg_if::cfg_if! {
    if #[cfg(target_arch = "wasm32")] {
        use web_time::Instant;
    } else {
        use std::time::Instant;
    }
}
use wgpu::{BindGroup, Buffer, Device, Queue, RenderPipeline, Surface, SurfaceConfiguration};
use winit::keyboard::ModifiersState;

use crate::canvas::{CanvasTransform, Uniforms};
use crate::drawing::{DrawingElement, Element, ElementId};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UserInputState {
    Idle,
    Panning,
    Drawing,
    Dragging,
    Resizing,
    MarqueeSelecting,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum ResizeHandle {
    NorthWest,
    North,
    NorthEast,
    East,
    SouthEast,
    South,
    SouthWest,
    West,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorPickerDragMode {
    HueRing,
    SvDisk,
}

#[derive(Debug, Clone)]
pub struct ColorPickerState {
    pub open: bool,
    pub hue: f32,
    pub saturation: f32,
    pub value: f32,
    pub drag_mode: Option<ColorPickerDragMode>,
}

impl ColorPickerState {
    pub fn new() -> Self {
        Self {
            open: false,
            hue: 0.0,
            saturation: 0.0,
            value: 0.0,
            drag_mode: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct SelectionState {
    pub selected_ids: Vec<ElementId>,
    pub active_handle: Option<ResizeHandle>,
    pub marquee_start: Option<[f32; 2]>,
    pub marquee_current: Option<[f32; 2]>,
    pub drag_origin: Option<[f32; 2]>,
    pub resize_bounds: Option<([f32; 2], [f32; 2])>,
    pub last_clicked: Option<(ElementId, Instant)>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self {
            selected_ids: Vec::new(),
            active_handle: None,
            marquee_start: None,
            marquee_current: None,
            drag_origin: None,
            resize_bounds: None,
            last_clicked: None,
        }
    }

    pub fn clear(&mut self) {
        self.selected_ids.clear();
        self.active_handle = None;
        self.marquee_start = None;
        self.marquee_current = None;
        self.drag_origin = None;
        self.resize_bounds = None;
    }

    pub fn is_selected(&self, id: ElementId) -> bool {
        self.selected_ids.contains(&id)
    }
}

pub struct GpuContext {
    pub surface: Surface<'static>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub render_pipeline: RenderPipeline,
    pub sdf_render_pipeline: RenderPipeline,
    pub ui_render_pipeline: RenderPipeline,
}

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct UiScreenUniforms {
    pub screen_size: [f32; 2],
    pub _padding: [f32; 2],
}

pub struct UiScreenBuffers {
    pub uniform: Buffer,
    pub bind_group: BindGroup,
}

pub struct Canvas {
    pub transform: CanvasTransform,
    pub uniform: Uniforms,
    pub uniform_buffer: Buffer,
    pub uniform_bind_group: BindGroup,
}

pub struct GeometryBuffers {
    pub vertex: Option<Buffer>,
    pub index: Option<Buffer>,
    pub count: u32,
}

pub struct UiBuffers {
    pub vertex: Option<Buffer>,
    pub index: Option<Buffer>,
    pub count: u32,
}

pub struct SdfBuffers {
    pub vertex: Option<Buffer>,
    pub index: Option<Buffer>,
    pub count: u32,
}

pub struct InputState {
    pub mouse_pos: [f32; 2],
    pub modifiers: ModifiersState,
    pub state: UserInputState,
    pub pan_start: Option<([f32; 2], [f32; 2])>,
    pub current_stroke: Vec<[f32; 2]>,
    pub drag_start: Option<[f32; 2]>,
    pub transform_snapshot: Vec<Element>,
    pub selection: SelectionState,
    pub preview_element: Option<DrawingElement>,
}

pub struct TextInput {
    pub active: bool,
    pub buffer: String,
    pub pos_canvas: [f32; 2],
    pub editing_id: Option<ElementId>,
    pub cursor_pos: usize,
    pub cursor_visible: bool,
    pub blink_timer: Instant,
}
