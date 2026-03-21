/// Visual style for vector path strokes.
#[derive(Debug, Clone, Copy)]
pub struct StrokeStyle {
    pub color: [f32; 4],
    pub width: f32,
}

impl StrokeStyle {
    pub fn new(color: [f32; 4], width: f32) -> Self {
        Self { color, width }
    }
}

/// Visual style for vector shape fills.
#[derive(Debug, Clone, Copy)]
pub struct FillStyle {
    pub color: [f32; 4],
}

impl FillStyle {
    pub fn new(color: [f32; 4]) -> Self {
        Self { color }
    }
}
