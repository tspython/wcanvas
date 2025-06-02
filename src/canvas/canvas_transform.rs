pub struct CanvasTransform {
    pub offset: [f32; 2],
    pub scale: f32,
}

impl CanvasTransform {
    pub fn new() -> Self {
        Self {
            offset: [0.0, 0.0],
            scale: 1.0,
        }
    }

    pub fn screen_to_canvas(&self, screen_pos: [f32; 2]) -> [f32; 2] {
        [
            (screen_pos[0] - self.offset[0]) / self.scale,
            (screen_pos[1] - self.offset[1]) / self.scale,
        ]
    }

    pub fn canvas_to_screen(&self, canvas_pos: [f32; 2]) -> [f32; 2] {
        [
            canvas_pos[0] * self.scale + self.offset[0],
            canvas_pos[1] * self.scale + self.offset[1],
        ]
    }
}
