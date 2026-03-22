use super::path::Path;
use super::style::StrokeStyle;
use crate::vertex::Vertex;

/// Converts vector paths into GPU-ready triangle geometry.
///
/// This is the single, reusable abstraction for converting any path
/// (strokes, rough shapes, selection highlights, etc.) into vertex/index
/// buffers. It replaces the duplicated line-to-quad expansion code that
/// was previously scattered throughout update_logic.rs.
pub struct PathTessellator {
    vertices: Vec<Vertex>,
    indices: Vec<u16>,
    index_offset: u16,
}

impl PathTessellator {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            index_offset: 0,
        }
    }

    /// Tessellate a path as a stroked polyline.
    ///
    /// Flattens curves to line segments, then expands each segment into
    /// a quad (two triangles) with the given stroke width.
    pub fn stroke(&mut self, path: &Path, style: &StrokeStyle) {
        let points = path.flatten(1.0);
        self.stroke_points(&points, style);
    }

    /// Tessellate a sequence of points as a stroked polyline.
    ///
    /// Each consecutive pair of points becomes a quad with the stroke width.
    pub fn stroke_points(&mut self, points: &[[f32; 2]], style: &StrokeStyle) {
        for i in 0..points.len().saturating_sub(1) {
            self.add_line_segment(points[i], points[i + 1], style.color, style.width);
        }
    }

    /// Tessellate a single line segment as a quad.
    pub fn add_line_segment(&mut self, p1: [f32; 2], p2: [f32; 2], color: [f32; 4], width: f32) {
        let dx = p2[0] - p1[0];
        let dy = p2[1] - p1[1];
        let len = (dx * dx + dy * dy).sqrt();

        if len <= 0.0 {
            return;
        }

        let nx = -dy / len * width * 0.5;
        let ny = dx / len * width * 0.5;

        self.vertices.extend_from_slice(&[
            Vertex {
                position: [p1[0] - nx, p1[1] - ny],
                color,
            },
            Vertex {
                position: [p1[0] + nx, p1[1] + ny],
                color,
            },
            Vertex {
                position: [p2[0] + nx, p2[1] + ny],
                color,
            },
            Vertex {
                position: [p2[0] - nx, p2[1] - ny],
                color,
            },
        ]);

        self.indices.extend_from_slice(&[
            self.index_offset,
            self.index_offset + 1,
            self.index_offset + 2,
            self.index_offset,
            self.index_offset + 2,
            self.index_offset + 3,
        ]);
        self.index_offset += 4;
    }

    /// Tessellate a filled convex polygon using a triangle fan from its center.
    pub fn fill_convex(&mut self, points: &[[f32; 2]], color: [f32; 4]) {
        if points.len() < 3 {
            return;
        }

        // Compute centroid
        let mut cx = 0.0f32;
        let mut cy = 0.0f32;
        for p in points {
            cx += p[0];
            cy += p[1];
        }
        let n = points.len() as f32;
        cx /= n;
        cy /= n;

        let center_idx = self.index_offset;
        self.vertices.push(Vertex {
            position: [cx, cy],
            color,
        });
        self.index_offset += 1;

        for p in points {
            self.vertices.push(Vertex {
                position: *p,
                color,
            });
        }

        let count = points.len() as u16;
        for i in 0..count {
            self.indices.extend_from_slice(&[
                center_idx,
                center_idx + 1 + i,
                center_idx + 1 + (i + 1) % count,
            ]);
        }
        self.index_offset += count;
    }

    /// Tessellate a stroked closed polygon (e.g. rectangle, diamond outline).
    pub fn stroke_polygon(&mut self, points: &[[f32; 2]], style: &StrokeStyle) {
        if points.is_empty() {
            return;
        }
        for i in 0..points.len() {
            let p1 = points[i];
            let p2 = points[(i + 1) % points.len()];
            self.add_line_segment(p1, p2, style.color, style.width);
        }
    }

    /// Consume the tessellator and return the accumulated geometry.
    pub fn finish(self) -> (Vec<Vertex>, Vec<u16>) {
        (self.vertices, self.indices)
    }

    /// Get current vertex/index counts (useful for merging with other geometry).
    pub fn counts(&self) -> (usize, usize) {
        (self.vertices.len(), self.indices.len())
    }

    /// Get the current index offset (useful for external geometry merging).
    pub fn index_offset(&self) -> u16 {
        self.index_offset
    }
}
