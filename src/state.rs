use std::time::Instant;
use wgpu::{BindGroup, Buffer, Device, Queue, RenderPipeline, Surface, SurfaceConfiguration};
use winit::keyboard::ModifiersState;

use crate::canvas::{CanvasTransform, Uniforms};

pub struct GpuContext<'a> {
    pub surface: Surface<'a>,
    pub device: Device,
    pub queue: Queue,
    pub config: SurfaceConfiguration,
    pub render_pipeline: RenderPipeline,
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

pub struct InputState {
    pub mouse_pos: [f32; 2],
    pub modifiers: ModifiersState,

    pub is_panning: bool,
    pub pan_start: Option<([f32; 2], [f32; 2])>,

    pub is_drawing: bool,
    pub current_stroke: Vec<[f32; 2]>,
    pub drag_start: Option<[f32; 2]>,
}

pub struct TextInput {
    pub active: bool,
    pub buffer: String,
    pub pos_canvas: [f32; 2],
    pub cursor_visible: bool,
    pub blink_timer: Instant,
}
