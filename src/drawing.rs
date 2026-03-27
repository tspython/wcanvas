use serde::{Deserialize, Serialize};
use std::sync::atomic::{AtomicU64, Ordering};

static NEXT_ELEMENT_ID: AtomicU64 = AtomicU64::new(1);
static NEXT_GROUP_ID: AtomicU64 = AtomicU64::new(1);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct ElementId(pub u64);

impl ElementId {
    pub fn next() -> Self {
        Self(NEXT_ELEMENT_ID.fetch_add(1, Ordering::Relaxed))
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
pub struct GroupId(pub u64);

impl GroupId {
    pub fn next() -> Self {
        Self(NEXT_GROUP_ID.fetch_add(1, Ordering::Relaxed))
    }
}

pub fn sync_id_counters(elements: &[Element]) {
    let next_element = elements
        .iter()
        .map(|element| element.id.0)
        .max()
        .unwrap_or(0)
        + 1;
    NEXT_ELEMENT_ID.fetch_max(next_element, Ordering::Relaxed);

    let next_group = elements
        .iter()
        .filter_map(|element| element.group_id.map(|group| group.0))
        .max()
        .unwrap_or(0)
        + 1;
    NEXT_GROUP_ID.fetch_max(next_group, Ordering::Relaxed);
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum Tool {
    Pen,
    Line,
    Rectangle,
    Circle,
    Diamond,
    Arrow,
    Text,
    Eraser,
    Select,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub enum BoxState {
    Idle,
    Selected,
    Editing,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Element {
    pub id: ElementId,
    pub group_id: Option<GroupId>,
    pub shape: DrawingElement,
}

impl Element {
    pub fn new(shape: DrawingElement) -> Self {
        Self {
            id: ElementId::next(),
            group_id: None,
            shape,
        }
    }

    pub fn with_group(mut self, group_id: GroupId) -> Self {
        self.group_id = Some(group_id);
        self
    }

    pub fn bounding_box(&self) -> ([f32; 2], [f32; 2]) {
        self.shape.bounding_box()
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DrawingElement {
    Stroke {
        points: Vec<[f32; 2]>,
        color: [f32; 4],
        width: f32,
    },
    Line {
        start: [f32; 2],
        end: [f32; 2],
        color: [f32; 4],
        width: f32,
        rough_style: Option<crate::rough::RoughOptions>,
    },
    Rectangle {
        position: [f32; 2],
        size: [f32; 2],
        #[serde(default)]
        rotation: f32,
        color: [f32; 4],
        fill: bool,
        stroke_width: f32,
        rough_style: Option<crate::rough::RoughOptions>,
    },
    Circle {
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        fill: bool,
        stroke_width: f32,
        rough_style: Option<crate::rough::RoughOptions>,
    },
    Diamond {
        position: [f32; 2],
        size: [f32; 2],
        #[serde(default)]
        rotation: f32,
        color: [f32; 4],
        fill: bool,
        stroke_width: f32,
        rough_style: Option<crate::rough::RoughOptions>,
    },
    Arrow {
        start: [f32; 2],
        end: [f32; 2],
        color: [f32; 4],
        width: f32,
        rough_style: Option<crate::rough::RoughOptions>,
    },
    Text {
        position: [f32; 2],
        content: String,
        color: [f32; 4],
        size: f32,
    },
    TextBox {
        id: u64,
        pos: [f32; 2],
        size: [f32; 2],
        content: String,
        color: [f32; 4],
        font_size: f32,
        state: BoxState,
    },
}

impl DrawingElement {
    pub fn color(&self) -> [f32; 4] {
        match self {
            DrawingElement::Stroke { color, .. }
            | DrawingElement::Line { color, .. }
            | DrawingElement::Rectangle { color, .. }
            | DrawingElement::Circle { color, .. }
            | DrawingElement::Diamond { color, .. }
            | DrawingElement::Arrow { color, .. }
            | DrawingElement::Text { color, .. }
            | DrawingElement::TextBox { color, .. } => *color,
        }
    }

    pub fn set_color(&mut self, color: [f32; 4]) {
        match self {
            DrawingElement::Stroke { color: value, .. }
            | DrawingElement::Line { color: value, .. }
            | DrawingElement::Rectangle { color: value, .. }
            | DrawingElement::Circle { color: value, .. }
            | DrawingElement::Diamond { color: value, .. }
            | DrawingElement::Arrow { color: value, .. }
            | DrawingElement::Text { color: value, .. }
            | DrawingElement::TextBox { color: value, .. } => *value = color,
        }
    }

    pub fn set_fill(&mut self, fill: bool) -> bool {
        match self {
            DrawingElement::Rectangle { fill: value, .. }
            | DrawingElement::Circle { fill: value, .. }
            | DrawingElement::Diamond { fill: value, .. } => {
                *value = fill;
                true
            }
            _ => false,
        }
    }

    pub fn toggle_fill(&mut self) -> bool {
        match self {
            DrawingElement::Rectangle { fill, .. }
            | DrawingElement::Circle { fill, .. }
            | DrawingElement::Diamond { fill, .. } => {
                *fill = !*fill;
                true
            }
            _ => false,
        }
    }

    pub fn set_stroke_width(&mut self, stroke_width: f32) -> bool {
        let stroke_width = stroke_width.max(0.5);
        match self {
            DrawingElement::Stroke { width, .. }
            | DrawingElement::Line { width, .. }
            | DrawingElement::Arrow { width, .. } => {
                *width = stroke_width;
                true
            }
            DrawingElement::Rectangle {
                stroke_width: value,
                rough_style,
                ..
            }
            | DrawingElement::Circle {
                stroke_width: value,
                rough_style,
                ..
            }
            | DrawingElement::Diamond {
                stroke_width: value,
                rough_style,
                ..
            } => {
                *value = stroke_width;
                if let Some(rough) = rough_style.as_mut() {
                    rough.stroke_width = stroke_width;
                }
                true
            }
            DrawingElement::Text { size, .. } => {
                *size = stroke_width.max(8.0);
                true
            }
            DrawingElement::TextBox { font_size, .. } => {
                *font_size = stroke_width.max(8.0);
                true
            }
        }
    }

    pub fn stroke_width(&self) -> f32 {
        match self {
            DrawingElement::Stroke { width, .. }
            | DrawingElement::Line { width, .. }
            | DrawingElement::Arrow { width, .. } => *width,
            DrawingElement::Rectangle { stroke_width, .. }
            | DrawingElement::Circle { stroke_width, .. }
            | DrawingElement::Diamond { stroke_width, .. } => *stroke_width,
            DrawingElement::Text { size, .. } => *size,
            DrawingElement::TextBox { font_size, .. } => *font_size,
        }
    }

    pub fn bounding_box(&self) -> ([f32; 2], [f32; 2]) {
        match self {
            DrawingElement::Stroke { points, width, .. } => {
                if points.is_empty() {
                    return ([0.0, 0.0], [0.0, 0.0]);
                }
                let mut min_x = points[0][0];
                let mut min_y = points[0][1];
                let mut max_x = points[0][0];
                let mut max_y = points[0][1];
                for point in points {
                    min_x = min_x.min(point[0]);
                    min_y = min_y.min(point[1]);
                    max_x = max_x.max(point[0]);
                    max_y = max_y.max(point[1]);
                }
                let padding = *width;
                (
                    [min_x - padding, min_y - padding],
                    [max_x + padding, max_y + padding],
                )
            }
            DrawingElement::Line {
                start, end, width, ..
            }
            | DrawingElement::Arrow {
                start, end, width, ..
            } => {
                let padding = *width * 2.0 + 4.0;
                (
                    [
                        start[0].min(end[0]) - padding,
                        start[1].min(end[1]) - padding,
                    ],
                    [
                        start[0].max(end[0]) + padding,
                        start[1].max(end[1]) + padding,
                    ],
                )
            }
            DrawingElement::Rectangle {
                position,
                size,
                rotation,
                stroke_width,
                ..
            } => {
                let padding = *stroke_width + 4.0;
                let corners = rectangle_corners(*position, *size, *rotation);
                let (mut min, mut max) = bounds_from_points(&corners);
                min[0] -= padding;
                min[1] -= padding;
                max[0] += padding;
                max[1] += padding;
                (min, max)
            }
            DrawingElement::Diamond {
                position,
                size,
                rotation,
                stroke_width,
                ..
            } => {
                let padding = *stroke_width + 4.0;
                let points = diamond_points(*position, *size, *rotation);
                let (mut min, mut max) = bounds_from_points(&points);
                min[0] -= padding;
                min[1] -= padding;
                max[0] += padding;
                max[1] += padding;
                (min, max)
            }
            DrawingElement::Circle {
                center,
                radius,
                stroke_width,
                ..
            } => {
                let padding = *stroke_width + 4.0;
                (
                    [center[0] - radius - padding, center[1] - radius - padding],
                    [center[0] + radius + padding, center[1] + radius + padding],
                )
            }
            DrawingElement::Text {
                position,
                content,
                size,
                ..
            } => {
                let width = text_width(content, *size);
                let height = *size * 1.2;
                (
                    [position[0], position[1] - height],
                    [position[0] + width, position[1]],
                )
            }
            DrawingElement::TextBox { pos, size, .. } => {
                ([pos[0], pos[1]], [pos[0] + size[0], pos[1] + size[1]])
            }
        }
    }

    pub fn center(&self) -> [f32; 2] {
        let (min, max) = self.bounding_box();
        [(min[0] + max[0]) * 0.5, (min[1] + max[1]) * 0.5]
    }

    pub fn translate_by(&mut self, dx: f32, dy: f32) {
        match self {
            DrawingElement::Text { position, .. } => {
                position[0] += dx;
                position[1] += dy;
            }
            DrawingElement::TextBox { pos, .. }
            | DrawingElement::Rectangle { position: pos, .. }
            | DrawingElement::Diamond { position: pos, .. } => {
                pos[0] += dx;
                pos[1] += dy;
            }
            DrawingElement::Circle { center, .. } => {
                center[0] += dx;
                center[1] += dy;
            }
            DrawingElement::Arrow { start, end, .. } | DrawingElement::Line { start, end, .. } => {
                start[0] += dx;
                start[1] += dy;
                end[0] += dx;
                end[1] += dy;
            }
            DrawingElement::Stroke { points, .. } => {
                for point in points {
                    point[0] += dx;
                    point[1] += dy;
                }
            }
        }
    }

    pub fn rotate_around(&mut self, pivot: [f32; 2], angle: f32) {
        if angle.abs() <= f32::EPSILON {
            return;
        }

        match self {
            DrawingElement::Rectangle {
                position,
                size,
                rotation,
                ..
            }
            | DrawingElement::Diamond {
                position,
                size,
                rotation,
                ..
            } => {
                let center = [position[0] + size[0] * 0.5, position[1] + size[1] * 0.5];
                let rotated_center = rotate_point(center, pivot, angle);
                position[0] = rotated_center[0] - size[0] * 0.5;
                position[1] = rotated_center[1] - size[1] * 0.5;
                *rotation = normalize_rotation(*rotation + angle);
            }
            DrawingElement::Circle { center, .. } => {
                *center = rotate_point(*center, pivot, angle);
            }
            DrawingElement::Arrow { start, end, .. } | DrawingElement::Line { start, end, .. } => {
                *start = rotate_point(*start, pivot, angle);
                *end = rotate_point(*end, pivot, angle);
            }
            DrawingElement::Stroke { points, .. } => {
                for point in points {
                    *point = rotate_point(*point, pivot, angle);
                }
            }
            DrawingElement::Text {
                position,
                content,
                size,
                ..
            } => {
                let width = text_width(content, *size);
                let height = *size * 1.2;
                let center = [position[0] + width * 0.5, position[1] - height * 0.5];
                let rotated_center = rotate_point(center, pivot, angle);
                position[0] = rotated_center[0] - width * 0.5;
                position[1] = rotated_center[1] + height * 0.5;
            }
            DrawingElement::TextBox { pos, size, .. } => {
                let center = [pos[0] + size[0] * 0.5, pos[1] + size[1] * 0.5];
                let rotated_center = rotate_point(center, pivot, angle);
                pos[0] = rotated_center[0] - size[0] * 0.5;
                pos[1] = rotated_center[1] - size[1] * 0.5;
            }
        }
    }

    pub fn hit_test(&self, pos: [f32; 2]) -> bool {
        match self {
            DrawingElement::Text {
                position,
                content,
                size,
                ..
            } => {
                let width = text_width(content, *size);
                let height = *size * 1.2;
                pos[0] >= position[0] - 5.0
                    && pos[0] <= position[0] + width + 5.0
                    && pos[1] >= position[1] - height
                    && pos[1] <= position[1] + 5.0
            }
            DrawingElement::TextBox {
                pos: element_pos,
                size,
                ..
            } => {
                pos[0] >= element_pos[0]
                    && pos[0] <= element_pos[0] + size[0]
                    && pos[1] >= element_pos[1]
                    && pos[1] <= element_pos[1] + size[1]
            }
            DrawingElement::Rectangle {
                position,
                size,
                rotation,
                ..
            }
            | DrawingElement::Diamond {
                position,
                size,
                rotation,
                ..
            } => {
                let center = [position[0] + size[0] * 0.5, position[1] + size[1] * 0.5];
                let local = rotate_point(pos, center, -*rotation);
                local[0] >= position[0]
                    && local[0] <= position[0] + size[0]
                    && local[1] >= position[1]
                    && local[1] <= position[1] + size[1]
            }
            DrawingElement::Circle { center, radius, .. } => {
                ((pos[0] - center[0]).powi(2) + (pos[1] - center[1]).powi(2)).sqrt() <= *radius
            }
            DrawingElement::Arrow {
                start, end, width, ..
            }
            | DrawingElement::Line {
                start, end, width, ..
            } => point_to_line_distance(pos, *start, *end) <= width * 2.0 + 4.0,
            DrawingElement::Stroke { points, width, .. } => points.windows(2).any(|segment| {
                point_to_line_distance(pos, segment[0], segment[1]) <= width * 2.0 + 4.0
            }),
        }
    }

    pub fn resize_to_bounds(
        &mut self,
        old_bounds: ([f32; 2], [f32; 2]),
        new_bounds: ([f32; 2], [f32; 2]),
        lock_aspect: bool,
    ) {
        let adjusted_bounds = if lock_aspect {
            lock_bounds_to_aspect(old_bounds, new_bounds)
        } else {
            new_bounds
        };

        let old_size = [
            (old_bounds.1[0] - old_bounds.0[0]).max(1.0),
            (old_bounds.1[1] - old_bounds.0[1]).max(1.0),
        ];
        let new_size = [
            (adjusted_bounds.1[0] - adjusted_bounds.0[0]).max(1.0),
            (adjusted_bounds.1[1] - adjusted_bounds.0[1]).max(1.0),
        ];
        let scale_x = new_size[0] / old_size[0];
        let scale_y = new_size[1] / old_size[1];

        match self {
            DrawingElement::Rectangle { position, size, .. }
            | DrawingElement::Diamond { position, size, .. }
            | DrawingElement::TextBox {
                pos: position,
                size,
                ..
            } => {
                *position = adjusted_bounds.0;
                *size = new_size;
            }
            DrawingElement::Circle { center, radius, .. } => {
                *center = [
                    (adjusted_bounds.0[0] + adjusted_bounds.1[0]) * 0.5,
                    (adjusted_bounds.0[1] + adjusted_bounds.1[1]) * 0.5,
                ];
                *radius = new_size[0].min(new_size[1]) * 0.5;
            }
            DrawingElement::Line { start, end, .. } | DrawingElement::Arrow { start, end, .. } => {
                *start = scale_point(*start, old_bounds, adjusted_bounds);
                *end = scale_point(*end, old_bounds, adjusted_bounds);
            }
            DrawingElement::Stroke { points, .. } => {
                for point in points {
                    *point = scale_point(*point, old_bounds, adjusted_bounds);
                }
            }
            DrawingElement::Text { position, size, .. } => {
                *position = scale_point(*position, old_bounds, adjusted_bounds);
                *size = (*size * scale_x.max(scale_y)).max(8.0);
            }
        }
    }
}

fn text_width(content: &str, size: f32) -> f32 {
    content
        .lines()
        .map(|line| line.chars().count() as f32)
        .fold(1.0, f32::max)
        * size
        * 0.6
}

fn point_to_line_distance(point: [f32; 2], line_start: [f32; 2], line_end: [f32; 2]) -> f32 {
    let line_length_squared =
        (line_end[0] - line_start[0]).powi(2) + (line_end[1] - line_start[1]).powi(2);

    if line_length_squared == 0.0 {
        return ((point[0] - line_start[0]).powi(2) + (point[1] - line_start[1]).powi(2)).sqrt();
    }

    let t = ((point[0] - line_start[0]) * (line_end[0] - line_start[0])
        + (point[1] - line_start[1]) * (line_end[1] - line_start[1]))
        / line_length_squared;
    let t = t.clamp(0.0, 1.0);
    let projection = [
        line_start[0] + t * (line_end[0] - line_start[0]),
        line_start[1] + t * (line_end[1] - line_start[1]),
    ];

    ((point[0] - projection[0]).powi(2) + (point[1] - projection[1]).powi(2)).sqrt()
}

fn scale_point(
    point: [f32; 2],
    old_bounds: ([f32; 2], [f32; 2]),
    new_bounds: ([f32; 2], [f32; 2]),
) -> [f32; 2] {
    let old_width = (old_bounds.1[0] - old_bounds.0[0]).max(1.0);
    let old_height = (old_bounds.1[1] - old_bounds.0[1]).max(1.0);
    let tx = (point[0] - old_bounds.0[0]) / old_width;
    let ty = (point[1] - old_bounds.0[1]) / old_height;
    [
        new_bounds.0[0] + tx * (new_bounds.1[0] - new_bounds.0[0]),
        new_bounds.0[1] + ty * (new_bounds.1[1] - new_bounds.0[1]),
    ]
}

fn lock_bounds_to_aspect(
    old_bounds: ([f32; 2], [f32; 2]),
    new_bounds: ([f32; 2], [f32; 2]),
) -> ([f32; 2], [f32; 2]) {
    let old_width = (old_bounds.1[0] - old_bounds.0[0]).max(1.0);
    let old_height = (old_bounds.1[1] - old_bounds.0[1]).max(1.0);
    let aspect = old_width / old_height;
    let mut width = new_bounds.1[0] - new_bounds.0[0];
    let mut height = new_bounds.1[1] - new_bounds.0[1];

    if width.abs() / height.abs().max(1.0) > aspect {
        width = height.abs() * aspect * width.signum();
    } else {
        height = width.abs() / aspect * height.signum();
    }

    (
        new_bounds.0,
        [new_bounds.0[0] + width, new_bounds.0[1] + height],
    )
}

fn rotate_point(point: [f32; 2], pivot: [f32; 2], angle: f32) -> [f32; 2] {
    let sin = angle.sin();
    let cos = angle.cos();
    let dx = point[0] - pivot[0];
    let dy = point[1] - pivot[1];
    [
        pivot[0] + dx * cos - dy * sin,
        pivot[1] + dx * sin + dy * cos,
    ]
}

fn normalize_rotation(angle: f32) -> f32 {
    let mut normalized = angle.rem_euclid(std::f32::consts::TAU);
    if normalized > std::f32::consts::PI {
        normalized -= std::f32::consts::TAU;
    }
    normalized
}

fn rectangle_corners(position: [f32; 2], size: [f32; 2], rotation: f32) -> [[f32; 2]; 4] {
    let center = [position[0] + size[0] * 0.5, position[1] + size[1] * 0.5];
    let mut corners = [
        position,
        [position[0] + size[0], position[1]],
        [position[0] + size[0], position[1] + size[1]],
        [position[0], position[1] + size[1]],
    ];
    if rotation.abs() > f32::EPSILON {
        for corner in &mut corners {
            *corner = rotate_point(*corner, center, rotation);
        }
    }
    corners
}

fn diamond_points(position: [f32; 2], size: [f32; 2], rotation: f32) -> [[f32; 2]; 4] {
    let center = [position[0] + size[0] * 0.5, position[1] + size[1] * 0.5];
    let half_w = size[0] * 0.5;
    let half_h = size[1] * 0.5;
    let mut points = [
        [center[0], center[1] - half_h],
        [center[0] + half_w, center[1]],
        [center[0], center[1] + half_h],
        [center[0] - half_w, center[1]],
    ];
    if rotation.abs() > f32::EPSILON {
        for point in &mut points {
            *point = rotate_point(*point, center, rotation);
        }
    }
    points
}

fn bounds_from_points(points: &[[f32; 2]]) -> ([f32; 2], [f32; 2]) {
    let mut min = [f32::INFINITY, f32::INFINITY];
    let mut max = [f32::NEG_INFINITY, f32::NEG_INFINITY];
    for point in points {
        min[0] = min[0].min(point[0]);
        min[1] = min[1].min(point[1]);
        max[0] = max[0].max(point[0]);
        max[1] = max[1].max(point[1]);
    }
    (min, max)
}
