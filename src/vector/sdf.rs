use crate::vertex::SdfVertex;

/// Shape types for SDF rendering. Must match the shader constants.
pub const SHAPE_RECT: f32 = 0.0;
pub const SHAPE_CIRCLE: f32 = 1.0;
pub const SHAPE_DIAMOND: f32 = 2.0;

/// Generates SDF quad geometry for resolution-independent shape rendering.
///
/// Instead of tessellating shapes into triangles, each shape is rendered as a
/// simple quad (bounding box). The fragment shader evaluates a signed distance
/// function to determine per-pixel coverage, producing perfectly smooth edges
/// at any zoom level.
pub struct SdfBatch {
    vertices: Vec<SdfVertex>,
    indices: Vec<u16>,
    index_offset: u16,
}

impl SdfBatch {
    pub fn new() -> Self {
        Self {
            vertices: Vec::new(),
            indices: Vec::new(),
            index_offset: 0,
        }
    }

    /// Add a rectangle shape.
    ///
    /// `position`: top-left corner in canvas space
    /// `size`: width and height in canvas space
    pub fn add_rect(
        &mut self,
        position: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
        stroke_width: f32,
        filled: bool,
    ) {
        let center = [position[0] + size[0] / 2.0, position[1] + size[1] / 2.0];
        let half_w = size[0] / 2.0;
        let half_h = size[1] / 2.0;
        let padding = stroke_width + 2.0;

        self.add_shape_quad(
            center,
            [half_w + padding, half_h + padding],
            color,
            SHAPE_RECT,
            half_w,
            half_h,
            stroke_width,
            filled,
        );
    }

    /// Add a circle shape.
    ///
    /// `center`: center position in canvas space
    /// `radius`: radius in canvas space
    pub fn add_circle(
        &mut self,
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
        stroke_width: f32,
        filled: bool,
    ) {
        let padding = stroke_width + 2.0;
        let extent = radius + padding;

        self.add_shape_quad(
            center,
            [extent, extent],
            color,
            SHAPE_CIRCLE,
            radius,
            radius,
            stroke_width,
            filled,
        );
    }

    /// Add a diamond shape.
    ///
    /// `position`: top-left of bounding box in canvas space
    /// `size`: width and height of bounding box
    pub fn add_diamond(
        &mut self,
        position: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
        stroke_width: f32,
        filled: bool,
    ) {
        let center = [position[0] + size[0] / 2.0, position[1] + size[1] / 2.0];
        let half_w = size[0] / 2.0;
        let half_h = size[1] / 2.0;
        let padding = stroke_width + 2.0;

        self.add_shape_quad(
            center,
            [half_w + padding, half_h + padding],
            color,
            SHAPE_DIAMOND,
            half_w,
            half_h,
            stroke_width,
            filled,
        );
    }

    /// Core method: add a quad for SDF rendering.
    ///
    /// The quad covers the bounding box of the shape plus padding.
    /// `local_pos` at each vertex encodes the offset from the shape center,
    /// which the fragment shader uses for SDF evaluation.
    fn add_shape_quad(
        &mut self,
        center: [f32; 2],
        extent: [f32; 2], // half-size of the quad (including padding)
        color: [f32; 4],
        shape_type: f32,
        half_w: f32,
        half_h: f32,
        stroke_width: f32,
        filled: bool,
    ) {
        let fill_flag = if filled { 1.0 } else { 0.0 };
        let shape_params = [shape_type, half_w, half_h, stroke_width];
        let fill_params = [fill_flag, 0.0, 0.0, 0.0];

        let corners = [
            (
                [-extent[0], -extent[1]],
                [center[0] - extent[0], center[1] - extent[1]],
            ),
            (
                [extent[0], -extent[1]],
                [center[0] + extent[0], center[1] - extent[1]],
            ),
            (
                [extent[0], extent[1]],
                [center[0] + extent[0], center[1] + extent[1]],
            ),
            (
                [-extent[0], extent[1]],
                [center[0] - extent[0], center[1] + extent[1]],
            ),
        ];

        for &(local, pos) in &corners {
            self.vertices.push(SdfVertex {
                position: pos,
                local_pos: local,
                color,
                shape_params,
                fill_params,
            });
        }

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

    pub fn is_empty(&self) -> bool {
        self.vertices.is_empty()
    }

    /// Consume the batch and return the accumulated geometry.
    pub fn finish(self) -> (Vec<SdfVertex>, Vec<u16>) {
        (self.vertices, self.indices)
    }
}
