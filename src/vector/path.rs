/// A 2D vector path composed of drawing commands.
///
/// Paths are the core vector primitive — all shapes (lines, rectangles, circles, etc.)
/// can be represented as paths. This provides a unified abstraction that the
/// `PathTessellator` can convert to GPU geometry.
#[derive(Debug, Clone)]
pub struct Path {
    commands: Vec<PathCommand>,
}

#[derive(Debug, Clone, Copy)]
pub enum PathCommand {
    MoveTo([f32; 2]),
    LineTo([f32; 2]),
    QuadTo {
        control: [f32; 2],
        end: [f32; 2],
    },
    CubicTo {
        c1: [f32; 2],
        c2: [f32; 2],
        end: [f32; 2],
    },
    Close,
}

impl Path {
    pub fn new() -> Self {
        Self {
            commands: Vec::new(),
        }
    }

    pub fn move_to(&mut self, p: [f32; 2]) -> &mut Self {
        self.commands.push(PathCommand::MoveTo(p));
        self
    }

    pub fn line_to(&mut self, p: [f32; 2]) -> &mut Self {
        self.commands.push(PathCommand::LineTo(p));
        self
    }

    pub fn quad_to(&mut self, control: [f32; 2], end: [f32; 2]) -> &mut Self {
        self.commands.push(PathCommand::QuadTo { control, end });
        self
    }

    pub fn cubic_to(&mut self, c1: [f32; 2], c2: [f32; 2], end: [f32; 2]) -> &mut Self {
        self.commands.push(PathCommand::CubicTo { c1, c2, end });
        self
    }

    pub fn close(&mut self) -> &mut Self {
        self.commands.push(PathCommand::Close);
        self
    }

    pub fn commands(&self) -> &[PathCommand] {
        &self.commands
    }

    /// Create a path from a polyline (sequence of points).
    pub fn from_points(points: &[[f32; 2]]) -> Self {
        let mut path = Self::new();
        if let Some((&first, rest)) = points.split_first() {
            path.move_to(first);
            for &p in rest {
                path.line_to(p);
            }
        }
        path
    }

    /// Create a closed polygon from a sequence of points.
    pub fn from_polygon(points: &[[f32; 2]]) -> Self {
        let mut path = Self::from_points(points);
        if points.len() > 2 {
            path.close();
        }
        path
    }

    /// Create a rectangle path.
    pub fn rect(position: [f32; 2], size: [f32; 2]) -> Self {
        let corners = [
            position,
            [position[0] + size[0], position[1]],
            [position[0] + size[0], position[1] + size[1]],
            [position[0], position[1] + size[1]],
        ];
        Self::from_polygon(&corners)
    }

    /// Create a circle path approximated by line segments.
    pub fn circle(center: [f32; 2], radius: f32, segments: u32) -> Self {
        let mut points = Vec::with_capacity(segments as usize);
        for i in 0..segments {
            let angle = (i as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            points.push([
                center[0] + angle.cos() * radius,
                center[1] + angle.sin() * radius,
            ]);
        }
        Self::from_polygon(&points)
    }

    /// Create a diamond path.
    pub fn diamond(position: [f32; 2], size: [f32; 2]) -> Self {
        let center_x = position[0] + size[0] / 2.0;
        let center_y = position[1] + size[1] / 2.0;
        let half_w = size[0] / 2.0;
        let half_h = size[1] / 2.0;

        let points = [
            [center_x, center_y - half_h],
            [center_x + half_w, center_y],
            [center_x, center_y + half_h],
            [center_x - half_w, center_y],
        ];
        Self::from_polygon(&points)
    }

    /// Create a line segment path.
    pub fn line(start: [f32; 2], end: [f32; 2]) -> Self {
        let mut path = Self::new();
        path.move_to(start);
        path.line_to(end);
        path
    }

    /// Create an arrow path (shaft + arrowhead).
    pub fn arrow(start: [f32; 2], end: [f32; 2], head_len: f32, head_angle: f32) -> Vec<Self> {
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let len = (dx * dx + dy * dy).sqrt();

        let mut paths = vec![Self::line(start, end)];

        if len > 0.0 {
            let dir_x = dx / len;
            let dir_y = dy / len;
            let cos_a = head_angle.cos();
            let sin_a = head_angle.sin();

            let left = [
                end[0] - head_len * (dir_x * cos_a - dir_y * sin_a),
                end[1] - head_len * (dir_y * cos_a + dir_x * sin_a),
            ];
            let right = [
                end[0] - head_len * (dir_x * cos_a + dir_y * sin_a),
                end[1] - head_len * (dir_y * cos_a - dir_x * sin_a),
            ];

            paths.push(Self::line(left, end));
            paths.push(Self::line(right, end));
        }

        paths
    }

    /// Flatten all curves in this path to line segments.
    pub fn flatten(&self, tolerance: f32) -> Vec<[f32; 2]> {
        let mut points = Vec::new();
        let mut current = [0.0f32; 2];
        let mut start = [0.0f32; 2];

        for cmd in &self.commands {
            match *cmd {
                PathCommand::MoveTo(p) => {
                    current = p;
                    start = p;
                    points.push(p);
                }
                PathCommand::LineTo(p) => {
                    current = p;
                    points.push(p);
                }
                PathCommand::QuadTo { control, end } => {
                    let steps = Self::quad_steps(current, control, end, tolerance);
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        let u = 1.0 - t;
                        let p = [
                            u * u * current[0] + 2.0 * u * t * control[0] + t * t * end[0],
                            u * u * current[1] + 2.0 * u * t * control[1] + t * t * end[1],
                        ];
                        points.push(p);
                    }
                    current = end;
                }
                PathCommand::CubicTo { c1, c2, end } => {
                    let steps = Self::cubic_steps(current, c1, c2, end, tolerance);
                    for i in 1..=steps {
                        let t = i as f32 / steps as f32;
                        let u = 1.0 - t;
                        let uu = u * u;
                        let uuu = uu * u;
                        let tt = t * t;
                        let ttt = tt * t;
                        let p = [
                            uuu * current[0]
                                + 3.0 * uu * t * c1[0]
                                + 3.0 * u * tt * c2[0]
                                + ttt * end[0],
                            uuu * current[1]
                                + 3.0 * uu * t * c1[1]
                                + 3.0 * u * tt * c2[1]
                                + ttt * end[1],
                        ];
                        points.push(p);
                    }
                    current = end;
                }
                PathCommand::Close => {
                    if current != start {
                        points.push(start);
                    }
                    current = start;
                }
            }
        }
        points
    }

    fn quad_steps(p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], _tolerance: f32) -> u32 {
        let dx = p2[0] - p0[0];
        let dy = p2[1] - p0[1];
        let len = (dx * dx + dy * dy).sqrt();
        (len / 4.0).max(4.0).min(32.0) as u32
    }

    fn cubic_steps(
        p0: [f32; 2],
        _p1: [f32; 2],
        _p2: [f32; 2],
        p3: [f32; 2],
        _tolerance: f32,
    ) -> u32 {
        let dx = p3[0] - p0[0];
        let dy = p3[1] - p0[1];
        let len = (dx * dx + dy * dy).sqrt();
        (len / 4.0).max(4.0).min(64.0) as u32
    }
}
