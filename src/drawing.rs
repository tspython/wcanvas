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
pub enum DrawingElement {
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
