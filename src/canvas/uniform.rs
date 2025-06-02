use crate::canvas::CanvasTransform;
use cgmath::SquareMatrix;
#[repr(C)]
#[derive(Debug, Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
pub struct Uniforms {
    transform: [[f32; 4]; 4],
}

impl Uniforms {
    pub fn new() -> Self {
        Self {
            transform: cgmath::Matrix4::identity().into(),
        }
    }

    pub fn update_transform(
        &mut self,
        canvas_transform: &CanvasTransform,
        window_size: (f32, f32),
    ) {
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
