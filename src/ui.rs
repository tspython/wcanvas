use crate::{
    drawing::Tool,
    state::{ColorPickerDragMode, ColorPickerState},
    vertex::UiVertex,
};

const BASE_PALETTE_SWATCH_SIZE: f32 = 36.0;
const BASE_PALETTE_PADDING: f32 = 10.0;
const CUSTOM_SWATCH_LABEL: [f32; 4] = [0.16, 0.18, 0.22, 1.0];
const PICKER_PANEL_BASE_SIZE: [f32; 2] = [320.0, 276.0];
const PICKER_MAX_SIZE: [f32; 2] = [560.0, 420.0];
const PICKER_MIN_SIZE: [f32; 2] = [280.0, 236.0];
const PICKER_HUE_BAR_WIDTH: f32 = 30.0;
const PICKER_PANEL_GAP: f32 = 18.0;
const PICKER_INSET: f32 = 18.0;
const TOOLBAR_BASE_ICON_SIZE: f32 = 36.0;
const TOOLBAR_BASE_SPACING: f32 = 12.0;
const TOOLBAR_BASE_PADDING: f32 = 20.0;

const PALETTE_COLORS: [[f32; 4]; 6] = [
    [0.0, 0.0, 0.0, 1.0],
    [0.15, 0.15, 0.15, 1.0],
    [0.83, 0.0, 0.25, 1.0],
    [0.0, 0.45, 0.95, 1.0],
    [1.0, 0.8, 0.0, 1.0],
    [0.14, 0.75, 0.37, 1.0],
];

pub struct UiRenderer {
    tool_icons: Vec<ToolIcon>,
    color_palette: Vec<ColorSwatch>,
}

struct ColorSwatch {
    color: [f32; 4],
    position: [f32; 2],
    size: [f32; 2],
}

struct ToolIcon {
    tool: Tool,
    position: [f32; 2],
    size: [f32; 2],
    key_binding: &'static str,
}

pub enum ColorInteraction {
    None,
    Color([f32; 4]),
    TogglePicker,
    BeginDrag(ColorPickerDragMode, [f32; 4]),
}

#[derive(Clone, Copy)]
struct UiLayout {
    screen_size: (f32, f32),
    scale: f32,
    edge_padding: f32,
    palette_origin: [f32; 2],
    swatch_size: f32,
    swatch_padding: f32,
    palette_size: [f32; 2],
    picker_origin: [f32; 2],
    picker_size: [f32; 2],
    sv_origin: [f32; 2],
    sv_size: [f32; 2],
    hue_origin: [f32; 2],
    hue_size: [f32; 2],
    toolbar_icon_size: f32,
    toolbar_spacing: f32,
    toolbar_padding: f32,
    toolbar_center: [f32; 2],
    toolbar_size: [f32; 2],
}

impl UiLayout {
    fn new(screen_size: (f32, f32)) -> Self {
        let short_side = screen_size.0.min(screen_size.1);
        let scale =
            ((screen_size.0 / 1440.0) * 0.55 + (screen_size.1 / 900.0) * 0.45).clamp(0.92, 1.55);
        let edge_padding = (screen_size.0 * 0.018).clamp(18.0, 38.0);
        let top_padding = titlebar_padding() + (screen_size.1 * 0.032).clamp(28.0, 54.0);

        let swatch_size = (short_side * 0.032).clamp(34.0, 56.0);
        let swatch_padding = (swatch_size * 0.24).clamp(8.0, 14.0);
        let palette_origin = [edge_padding, top_padding + 26.0];
        let palette_width = swatch_size * 2.0 + swatch_padding;
        let palette_height = swatch_size * 4.0 + swatch_padding * 3.0;

        let desired_picker_width = (screen_size.0 * 0.22).max(PICKER_PANEL_BASE_SIZE[0] * scale);
        let mut picker_width = desired_picker_width.clamp(PICKER_MIN_SIZE[0], PICKER_MAX_SIZE[0]);
        let mut picker_height = (picker_width * 0.82).clamp(PICKER_MIN_SIZE[1], PICKER_MAX_SIZE[1]);
        let horizontal_origin = [
            palette_origin[0] + palette_width + PICKER_PANEL_GAP,
            top_padding + 12.0,
        ];

        let stack_picker = horizontal_origin[0] + picker_width + edge_padding > screen_size.0;
        let picker_origin = if stack_picker {
            picker_width =
                (screen_size.0 - edge_padding * 2.0).clamp(PICKER_MIN_SIZE[0], PICKER_MAX_SIZE[0]);
            picker_height = (picker_width * 0.72).clamp(PICKER_MIN_SIZE[1], PICKER_MAX_SIZE[1]);
            [
                edge_padding,
                palette_origin[1] + palette_height + PICKER_PANEL_GAP,
            ]
        } else {
            horizontal_origin
        };

        let inset = (PICKER_INSET * scale).clamp(16.0, 28.0);
        let hue_width = (PICKER_HUE_BAR_WIDTH * scale).clamp(24.0, 36.0);
        let preview_height = (picker_height * 0.11).clamp(24.0, 42.0);
        let sv_origin = [
            picker_origin[0] + inset,
            picker_origin[1] + inset + preview_height + 14.0,
        ];
        let sv_size = [
            picker_width - inset * 3.0 - hue_width,
            picker_height - inset * 2.0 - preview_height - 14.0,
        ];
        let hue_origin = [sv_origin[0] + sv_size[0] + inset, sv_origin[1]];
        let hue_size = [hue_width, sv_size[1]];

        let toolbar_icon_size = (TOOLBAR_BASE_ICON_SIZE * scale).clamp(34.0, 52.0);
        let toolbar_spacing = (TOOLBAR_BASE_SPACING * scale).clamp(10.0, 18.0);
        let toolbar_padding = (TOOLBAR_BASE_PADDING * scale).clamp(18.0, 30.0);
        let toolbar_width = 9.0 * toolbar_icon_size + 8.0 * toolbar_spacing + 2.0 * toolbar_padding;
        let toolbar_height = toolbar_icon_size + 2.0 * toolbar_padding;
        let toolbar_center = [screen_size.0 * 0.5, top_padding];

        Self {
            screen_size,
            scale,
            edge_padding,
            palette_origin,
            swatch_size,
            swatch_padding,
            palette_size: [palette_width, palette_height],
            picker_origin,
            picker_size: [picker_width, picker_height],
            sv_origin,
            sv_size,
            hue_origin,
            hue_size,
            toolbar_icon_size,
            toolbar_spacing,
            toolbar_padding,
            toolbar_center,
            toolbar_size: [toolbar_width, toolbar_height],
        }
    }

    fn custom_swatch_center(&self) -> [f32; 2] {
        [
            self.palette_origin[0] + self.swatch_size * 0.5,
            self.palette_origin[1]
                + 3.0 * (self.swatch_size + self.swatch_padding)
                + self.swatch_size * 0.5,
        ]
    }

    fn picker_center(&self) -> [f32; 2] {
        [
            self.picker_origin[0] + self.picker_size[0] * 0.5,
            self.picker_origin[1] + self.picker_size[1] * 0.5,
        ]
    }

    fn zoom_center(&self) -> [f32; 2] {
        [
            self.edge_padding + self.zoom_size()[0] * 0.5,
            self.screen_size.1 - self.edge_padding - self.zoom_size()[1] * 0.5,
        ]
    }

    fn zoom_size(&self) -> [f32; 2] {
        [
            (96.0 * self.scale).clamp(88.0, 144.0),
            (32.0 * self.scale).clamp(28.0, 46.0),
        ]
    }

    fn zoom_text_pos(&self) -> [f32; 2] {
        let center = self.zoom_center();
        [
            center[0] - self.zoom_size()[0] * 0.28,
            center[1] + self.zoom_size()[1] * 0.13,
        ]
    }

    fn zoom_font_size(&self) -> f32 {
        (15.0 * self.scale).clamp(13.0, 22.0)
    }
}

impl UiRenderer {
    pub fn new() -> Self {
        let tool_icons = vec![
            ToolIcon {
                tool: Tool::Select,
                position: [10.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "1",
            },
            ToolIcon {
                tool: Tool::Pen,
                position: [60.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "2",
            },
            ToolIcon {
                tool: Tool::Rectangle,
                position: [110.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "3",
            },
            ToolIcon {
                tool: Tool::Circle,
                position: [160.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "4",
            },
            ToolIcon {
                tool: Tool::Diamond,
                position: [210.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "5",
            },
            ToolIcon {
                tool: Tool::Arrow,
                position: [260.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "6",
            },
            ToolIcon {
                tool: Tool::Text,
                position: [310.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "7",
            },
            ToolIcon {
                tool: Tool::Line,
                position: [360.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "8",
            },
            ToolIcon {
                tool: Tool::Eraser,
                position: [410.0, 10.0],
                size: [40.0, 40.0],
                key_binding: "9",
            },
        ];

        let color_palette = PALETTE_COLORS
            .iter()
            .enumerate()
            .map(|(i, &color)| {
                let cols = 2;
                let swatch_size = 30.0;
                let padding = 8.0;
                let start_x = 15.0;
                let start_y = 90.0;

                let col = (i % cols) as f32;
                let row = (i / cols) as f32;

                ColorSwatch {
                    color,
                    position: [
                        start_x + col as f32 * (swatch_size + padding),
                        start_y + row as f32 * (swatch_size + padding),
                    ],
                    size: [swatch_size, swatch_size],
                }
            })
            .collect();

        Self {
            tool_icons,
            color_palette,
        }
    }

    fn draw_tool_icon(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        tool: Tool,
        center: [f32; 2],
        size: f32,
        color: [f32; 4],
    ) {
        match tool {
            Tool::Select => {
                let cursor_size = size * 0.8;
                let tip = [center[0] - cursor_size * 0.3, center[1] - cursor_size * 0.3];
                let left_base = [tip[0] + cursor_size * 0.15, tip[1] + cursor_size * 0.7];
                let right_base = [tip[0] + cursor_size * 0.6, tip[1] + cursor_size * 0.4];
                let notch = [tip[0] + cursor_size * 0.35, tip[1] + cursor_size * 0.55];

                self.create_simple_triangle(
                    vertices,
                    indices,
                    index_offset,
                    tip,
                    left_base,
                    notch,
                    color,
                );
                self.create_simple_triangle(
                    vertices,
                    indices,
                    index_offset,
                    tip,
                    notch,
                    right_base,
                    color,
                );
            }
            Tool::Pen => {
                let line_size = size * 0.7;
                let thickness = size * 0.135;

                let start = [center[0] - line_size * 0.3, center[1] - line_size * 0.3];
                let end = [center[0] + line_size * 0.3, center[1] + line_size * 0.3];

                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let len = (dx * dx + dy * dy).sqrt();
                let perp_x = -dy / len * thickness * 0.5;
                let perp_y = dx / len * thickness * 0.5;

                vertices.extend_from_slice(&[
                    UiVertex {
                        position: [start[0] + perp_x, start[1] + perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                    UiVertex {
                        position: [start[0] - perp_x, start[1] - perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                    UiVertex {
                        position: [end[0] - perp_x, end[1] - perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                    UiVertex {
                        position: [end[0] + perp_x, end[1] + perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                ]);

                indices.extend_from_slice(&[
                    *index_offset,
                    *index_offset + 1,
                    *index_offset + 2,
                    *index_offset,
                    *index_offset + 2,
                    *index_offset + 3,
                ]);
                *index_offset += 4;
            }
            Tool::Rectangle => {
                let rect_size = size * 0.6;
                let thickness = size * 0.162;
                self.draw_rect_outline(
                    vertices,
                    indices,
                    index_offset,
                    center,
                    [rect_size, rect_size * 0.7],
                    thickness,
                    color,
                );
            }
            Tool::Circle => {
                let radius = size * 0.35;
                let thickness = size * 0.162;
                self.draw_smooth_circle_outline(
                    vertices,
                    indices,
                    index_offset,
                    center,
                    radius,
                    thickness,
                    color,
                );
            }
            Tool::Diamond => {
                let diamond_size = size * 0.7;
                let diamond_thickness = size * 0.162;
                self.draw_smooth_diamond_outline(
                    vertices,
                    indices,
                    index_offset,
                    center,
                    diamond_size,
                    diamond_thickness,
                    color,
                );
            }
            Tool::Arrow => {
                let arrow_length = size * 0.7;
                let arrow_width = size * 0.25;
                self.draw_clean_arrow(
                    vertices,
                    indices,
                    index_offset,
                    center,
                    arrow_length,
                    arrow_width,
                    color,
                );
            }
            Tool::Text => {
                let t_size = size * 0.7;
                let thickness = size * 0.16;

                self.create_simple_rect(
                    vertices,
                    indices,
                    index_offset,
                    [center[0], center[1] - t_size * 0.35],
                    [t_size * 0.8, thickness],
                    color,
                );

                self.create_simple_rect(
                    vertices,
                    indices,
                    index_offset,
                    [center[0], center[1] + thickness * 0.5],
                    [thickness, t_size * 0.8],
                    color,
                );
            }
            Tool::Eraser => {
                let eraser_width = size * 0.4;
                let eraser_height = size * 0.6;

                self.create_simple_rect(
                    vertices,
                    indices,
                    index_offset,
                    center,
                    [eraser_width, eraser_height],
                    color,
                );

                let band_center = [center[0], center[1] - eraser_height * 0.15];
                self.create_simple_rect(
                    vertices,
                    indices,
                    index_offset,
                    band_center,
                    [eraser_width * 1.1, eraser_height * 0.15],
                    color,
                );
            }
            Tool::Line => {
                let line_size = size * 0.7;
                let thickness = size * 0.135;

                let start = [center[0] - line_size * 0.4, center[1] - line_size * 0.2];
                let end = [center[0] + line_size * 0.4, center[1] + line_size * 0.2];

                let dx = end[0] - start[0];
                let dy = end[1] - start[1];
                let len = (dx * dx + dy * dy).sqrt();
                let perp_x = -dy / len * thickness * 0.5;
                let perp_y = dx / len * thickness * 0.5;

                vertices.extend_from_slice(&[
                    UiVertex {
                        position: [start[0] + perp_x, start[1] + perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                    UiVertex {
                        position: [start[0] - perp_x, start[1] - perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                    UiVertex {
                        position: [end[0] - perp_x, end[1] - perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                    UiVertex {
                        position: [end[0] + perp_x, end[1] + perp_y],
                        color,
                        uv: [0.0, 0.0],
                    },
                ]);

                indices.extend_from_slice(&[
                    *index_offset,
                    *index_offset + 1,
                    *index_offset + 2,
                    *index_offset,
                    *index_offset + 2,
                    *index_offset + 3,
                ]);
                *index_offset += 4;
            }
        }
    }

    fn create_rounded_rect(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
        _corner_radius: f32,
        _border_width: f32,
    ) {
        let half_width = size[0] * 0.5;
        let half_height = size[1] * 0.5;

        let x0 = center[0] - half_width;
        let y0 = center[1] - half_height;
        let x1 = center[0] + half_width;
        let y1 = center[1] + half_height;

        vertices.extend_from_slice(&[
            UiVertex {
                position: [x0, y0],
                color,
                uv: [-1.0, -1.0],
            },
            UiVertex {
                position: [x1, y0],
                color,
                uv: [1.0, -1.0],
            },
            UiVertex {
                position: [x1, y1],
                color,
                uv: [1.0, 1.0],
            },
            UiVertex {
                position: [x0, y1],
                color,
                uv: [-1.0, 1.0],
            },
        ]);

        indices.extend_from_slice(&[
            *index_offset,
            *index_offset + 1,
            *index_offset + 2,
            *index_offset,
            *index_offset + 2,
            *index_offset + 3,
        ]);
        *index_offset += 4;
    }

    fn create_simple_rect(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: [f32; 2],
        color: [f32; 4],
    ) {
        let half_width = size[0] * 0.5;
        let half_height = size[1] * 0.5;

        vertices.extend_from_slice(&[
            UiVertex {
                position: [center[0] - half_width, center[1] - half_height],
                color,
                uv: [-1.0, -1.0],
            },
            UiVertex {
                position: [center[0] + half_width, center[1] - half_height],
                color,
                uv: [1.0, -1.0],
            },
            UiVertex {
                position: [center[0] + half_width, center[1] + half_height],
                color,
                uv: [1.0, 1.0],
            },
            UiVertex {
                position: [center[0] - half_width, center[1] + half_height],
                color,
                uv: [-1.0, 1.0],
            },
        ]);

        indices.extend_from_slice(&[
            *index_offset,
            *index_offset + 1,
            *index_offset + 2,
            *index_offset,
            *index_offset + 2,
            *index_offset + 3,
        ]);
        *index_offset += 4;
    }

    fn create_simple_triangle(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        p1: [f32; 2],
        p2: [f32; 2],
        p3: [f32; 2],
        color: [f32; 4],
    ) {
        vertices.extend_from_slice(&[
            UiVertex {
                position: p1,
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: p2,
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: p3,
                color,
                uv: [0.0, 0.0],
            },
        ]);

        indices.extend_from_slice(&[*index_offset, *index_offset + 1, *index_offset + 2]);
        *index_offset += 3;
    }

    fn create_colored_triangle(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        p1: [f32; 2],
        c1: [f32; 4],
        p2: [f32; 2],
        c2: [f32; 4],
        p3: [f32; 2],
        c3: [f32; 4],
    ) {
        vertices.extend_from_slice(&[
            UiVertex {
                position: p1,
                color: c1,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: p2,
                color: c2,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: p3,
                color: c3,
                uv: [0.0, 0.0],
            },
        ]);

        indices.extend_from_slice(&[*index_offset, *index_offset + 1, *index_offset + 2]);
        *index_offset += 3;
    }

    fn draw_filled_circle(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        radius: f32,
        color: [f32; 4],
    ) {
        let segments = 32;
        for i in 0..segments {
            let angle1 = (i as f32 / segments as f32) * std::f32::consts::TAU;
            let angle2 = ((i + 1) as f32 / segments as f32) * std::f32::consts::TAU;
            let p1 = [
                center[0] + radius * angle1.cos(),
                center[1] + radius * angle1.sin(),
            ];
            let p2 = [
                center[0] + radius * angle2.cos(),
                center[1] + radius * angle2.sin(),
            ];
            self.create_simple_triangle(vertices, indices, index_offset, center, p1, p2, color);
        }
    }

    fn draw_rect_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: [f32; 2],
        thickness: f32,
        color: [f32; 4],
    ) {
        let half_w = size[0] * 0.5;
        let half_h = size[1] * 0.5;
        let half_t = thickness * 0.5;

        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            [center[0], center[1] - half_h],
            [size[0], thickness],
            color,
        );
        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            [center[0], center[1] + half_h],
            [size[0], thickness],
            color,
        );
        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            [center[0] - half_w, center[1]],
            [thickness, size[1]],
            color,
        );
        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            [center[0] + half_w, center[1]],
            [thickness, size[1]],
            color,
        );
    }

    fn draw_circle_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        radius: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let segments = 12;
        for i in 0..segments {
            let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / segments as f32;

            let x1 = center[0] + angle1.cos() * radius;
            let y1 = center[1] + angle1.sin() * radius;
            let x2 = center[0] + angle2.cos() * radius;
            let y2 = center[1] + angle2.sin() * radius;

            let dx = x2 - x1;
            let dy = y2 - y1;
            let len = (dx * dx + dy * dy).sqrt();
            if len > 0.0 {
                let segment_center = [(x1 + x2) * 0.5, (y1 + y2) * 0.5];
                self.create_simple_rect(
                    vertices,
                    indices,
                    index_offset,
                    segment_center,
                    [len, thickness],
                    color,
                );
            }
        }
    }

    fn draw_arrow_shape(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        length: f32,
        width: f32,
        color: [f32; 4],
    ) {
        let shaft_width = width * 0.3;
        let head_width = width;
        let head_length = length * 0.4;
        let shaft_length = length - head_length;

        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            [center[0] - shaft_length * 0.25, center[1]],
            [shaft_length * 0.5, shaft_width],
            color,
        );

        let tip = [center[0] + length * 0.4, center[1]];
        let base1 = [center[0], center[1] - head_width * 0.5];
        let base2 = [center[0], center[1] + head_width * 0.5];
        self.create_simple_triangle(vertices, indices, index_offset, tip, base1, base2, color);
    }

    fn draw_t_shape(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        width: f32,
        height: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            [center[0], center[1] - height * 0.3],
            [width, thickness],
            color,
        );

        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            center,
            [thickness, height],
            color,
        );
    }

    fn draw_smooth_circle_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        radius: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let segments = 24;
        let inner_radius = radius - thickness * 0.5;
        let outer_radius = radius + thickness * 0.5;

        for i in 0..segments {
            let angle1 = (i as f32 * 2.0 * std::f32::consts::PI) / segments as f32;
            let angle2 = ((i + 1) as f32 * 2.0 * std::f32::consts::PI) / segments as f32;

            let inner1 = [
                center[0] + angle1.cos() * inner_radius,
                center[1] + angle1.sin() * inner_radius,
            ];
            let outer1 = [
                center[0] + angle1.cos() * outer_radius,
                center[1] + angle1.sin() * outer_radius,
            ];
            let inner2 = [
                center[0] + angle2.cos() * inner_radius,
                center[1] + angle2.sin() * inner_radius,
            ];
            let outer2 = [
                center[0] + angle2.cos() * outer_radius,
                center[1] + angle2.sin() * outer_radius,
            ];

            vertices.extend_from_slice(&[
                UiVertex {
                    position: inner1,
                    color,
                    uv: [0.0, 0.0],
                },
                UiVertex {
                    position: outer1,
                    color,
                    uv: [0.0, 0.0],
                },
                UiVertex {
                    position: outer2,
                    color,
                    uv: [0.0, 0.0],
                },
                UiVertex {
                    position: inner2,
                    color,
                    uv: [0.0, 0.0],
                },
            ]);

            indices.extend_from_slice(&[
                *index_offset,
                *index_offset + 1,
                *index_offset + 2,
                *index_offset,
                *index_offset + 2,
                *index_offset + 3,
            ]);
            *index_offset += 4;
        }
    }

    fn draw_smooth_diamond_outline(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: f32,
        thickness: f32,
        color: [f32; 4],
    ) {
        let half_size = size * 0.5;

        let top = [center[0], center[1] - half_size];
        let right = [center[0] + half_size, center[1]];
        let bottom = [center[0], center[1] + half_size];
        let left = [center[0] - half_size, center[1]];

        self.draw_thick_line(
            vertices,
            indices,
            index_offset,
            top,
            right,
            thickness,
            color,
        );
        self.draw_thick_line(
            vertices,
            indices,
            index_offset,
            right,
            bottom,
            thickness,
            color,
        );
        self.draw_thick_line(
            vertices,
            indices,
            index_offset,
            bottom,
            left,
            thickness,
            color,
        );
        self.draw_thick_line(vertices, indices, index_offset, left, top, thickness, color);
    }

    fn draw_thick_line(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        start: [f32; 2],
        end: [f32; 2],
        thickness: f32,
        color: [f32; 4],
    ) {
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let len = (dx * dx + dy * dy).sqrt();

        if len > 0.0 {
            let perp_x = -dy / len * thickness * 0.5;
            let perp_y = dx / len * thickness * 0.5;

            vertices.extend_from_slice(&[
                UiVertex {
                    position: [start[0] + perp_x, start[1] + perp_y],
                    color,
                    uv: [0.0, 0.0],
                },
                UiVertex {
                    position: [start[0] - perp_x, start[1] - perp_y],
                    color,
                    uv: [0.0, 0.0],
                },
                UiVertex {
                    position: [end[0] - perp_x, end[1] - perp_y],
                    color,
                    uv: [0.0, 0.0],
                },
                UiVertex {
                    position: [end[0] + perp_x, end[1] + perp_y],
                    color,
                    uv: [0.0, 0.0],
                },
            ]);

            indices.extend_from_slice(&[
                *index_offset,
                *index_offset + 1,
                *index_offset + 2,
                *index_offset,
                *index_offset + 2,
                *index_offset + 3,
            ]);
            *index_offset += 4;
        }
    }

    fn draw_clean_arrow(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        length: f32,
        width: f32,
        color: [f32; 4],
    ) {
        let shaft_width = width * 0.4;
        let head_width = width;
        let head_length = length * 0.4;
        let shaft_length = length - head_length;

        let shaft_center = [center[0] - head_length * 0.25, center[1]];
        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            shaft_center,
            [shaft_length * 0.5, shaft_width],
            color,
        );

        let tip = [center[0] + length * 0.4, center[1]];
        let base1 = [center[0] - head_length * 0.1, center[1] - head_width * 0.5];
        let base2 = [center[0] - head_length * 0.1, center[1] + head_width * 0.5];
        self.create_simple_triangle(vertices, indices, index_offset, tip, base1, base2, color);
    }

    fn draw_text_icon(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: f32,
        color: [f32; 4],
    ) {
        let thickness = size * 0.18;
        let height = size * 0.9;
        let width = size * 0.8;

        let left_angle = 0.3;
        let left_center = [center[0] - width * 0.25, center[1] + height * 0.1];
        let left_length = height * 0.8;

        let cos_a = (left_angle as f32).cos();
        let sin_a = (left_angle as f32).sin();
        let half_thick = thickness * 0.5;
        let half_len = left_length * 0.5;

        let left_corners = [
            [
                left_center[0] - cos_a * half_len + sin_a * half_thick,
                left_center[1] - sin_a * half_len - cos_a * half_thick,
            ],
            [
                left_center[0] + cos_a * half_len + sin_a * half_thick,
                left_center[1] + sin_a * half_len - cos_a * half_thick,
            ],
            [
                left_center[0] + cos_a * half_len - sin_a * half_thick,
                left_center[1] + sin_a * half_len + cos_a * half_thick,
            ],
            [
                left_center[0] - cos_a * half_len - sin_a * half_thick,
                left_center[1] - sin_a * half_len + cos_a * half_thick,
            ],
        ];

        vertices.extend_from_slice(&[
            UiVertex {
                position: left_corners[0],
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: left_corners[1],
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: left_corners[2],
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: left_corners[3],
                color,
                uv: [0.0, 0.0],
            },
        ]);

        indices.extend_from_slice(&[
            *index_offset,
            *index_offset + 1,
            *index_offset + 2,
            *index_offset,
            *index_offset + 2,
            *index_offset + 3,
        ]);
        *index_offset += 4;

        let right_center = [center[0] + width * 0.25, center[1] + height * 0.1];
        let right_corners = [
            [
                right_center[0] + cos_a * half_len + sin_a * half_thick,
                right_center[1] - sin_a * half_len - cos_a * half_thick,
            ],
            [
                right_center[0] - cos_a * half_len + sin_a * half_thick,
                right_center[1] + sin_a * half_len - cos_a * half_thick,
            ],
            [
                right_center[0] - cos_a * half_len - sin_a * half_thick,
                right_center[1] + sin_a * half_len + cos_a * half_thick,
            ],
            [
                right_center[0] + cos_a * half_len - sin_a * half_thick,
                right_center[1] - sin_a * half_len + cos_a * half_thick,
            ],
        ];

        vertices.extend_from_slice(&[
            UiVertex {
                position: right_corners[0],
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: right_corners[1],
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: right_corners[2],
                color,
                uv: [0.0, 0.0],
            },
            UiVertex {
                position: right_corners[3],
                color,
                uv: [0.0, 0.0],
            },
        ]);

        indices.extend_from_slice(&[
            *index_offset,
            *index_offset + 1,
            *index_offset + 2,
            *index_offset,
            *index_offset + 2,
            *index_offset + 3,
        ]);
        *index_offset += 4;

        let crossbar_center = [center[0], center[1] + height * 0.15];
        self.create_simple_rect(
            vertices,
            indices,
            index_offset,
            crossbar_center,
            [width * 0.5, thickness],
            color,
        );
    }

    fn generate_color_picker(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        current_color: [f32; 4],
        picker: &ColorPickerState,
        screen_size: (f32, f32),
    ) {
        let layout = UiLayout::new(screen_size);
        let palette_panel_size = [
            layout.palette_size[0] + layout.edge_padding * 0.75,
            layout.palette_size[1] + layout.edge_padding * 0.65,
        ];
        let palette_panel_center = [
            layout.palette_origin[0] + layout.palette_size[0] * 0.5,
            layout.palette_origin[1] + layout.palette_size[1] * 0.5,
        ];

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [palette_panel_center[0] + 3.0, palette_panel_center[1] + 5.0],
            palette_panel_size,
            [0.02, 0.03, 0.05, 0.10],
            14.0,
            0.0,
        );
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            palette_panel_center,
            palette_panel_size,
            [0.95, 0.96, 0.98, 0.96],
            14.0,
            1.0,
        );

        for (i, color) in PALETTE_COLORS.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;

            let center = [
                layout.palette_origin[0]
                    + col * (layout.swatch_size + layout.swatch_padding)
                    + layout.swatch_size * 0.5,
                layout.palette_origin[1]
                    + row * (layout.swatch_size + layout.swatch_padding)
                    + layout.swatch_size * 0.5,
            ];

            let is_selected = (color[0] - current_color[0]).abs() < 0.01
                && (color[1] - current_color[1]).abs() < 0.01
                && (color[2] - current_color[2]).abs() < 0.01;

            let border_width = if is_selected { 2.0 } else { 1.0 };
            let final_color = if is_selected {
                [color[0] * 1.1, color[1] * 1.1, color[2] * 1.1, color[3]]
            } else {
                *color
            };

            self.create_rounded_rect(
                vertices,
                indices,
                index_offset,
                center,
                [layout.swatch_size, layout.swatch_size],
                final_color,
                8.0,
                border_width,
            );
        }

        let custom_center = layout.custom_swatch_center();
        let custom_color = self.color_from_picker(picker);
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            custom_center,
            [layout.swatch_size, layout.swatch_size],
            custom_color,
            8.0,
            if picker.open { 2.0 } else { 1.0 },
        );
        self.draw_plus_icon(
            vertices,
            indices,
            index_offset,
            custom_center,
            layout.swatch_size * 0.44,
            CUSTOM_SWATCH_LABEL,
        );

        if !picker.open {
            return;
        }

        let picker_center = layout.picker_center();
        let preview_center = [
            picker_center[0],
            layout.picker_origin[1] + layout.picker_size[1] * 0.12,
        ];

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [picker_center[0] + 4.0, picker_center[1] + 6.0],
            layout.picker_size,
            [0.04, 0.05, 0.08, 0.12],
            18.0,
            0.0,
        );
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            picker_center,
            layout.picker_size,
            [0.93, 0.93, 0.95, 0.98],
            18.0,
            1.0,
        );
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            preview_center,
            [layout.picker_size[0] - 26.0, layout.picker_size[1] * 0.16],
            [0.98, 0.985, 0.99, 0.85],
            14.0,
            0.0,
        );
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            preview_center,
            [layout.picker_size[0] - 40.0, layout.picker_size[1] * 0.10],
            custom_color,
            12.0,
            0.0,
        );

        self.generate_sv_panel(vertices, indices, index_offset, picker, layout);
        self.generate_hue_slider(vertices, indices, index_offset, picker, layout);

        self.draw_filled_circle(
            vertices,
            indices,
            index_offset,
            self.hue_knob_position(picker, layout),
            5.5,
            [1.0, 1.0, 1.0, 1.0],
        );
        self.draw_smooth_circle_outline(
            vertices,
            indices,
            index_offset,
            self.hue_knob_position(picker, layout),
            6.0,
            1.5,
            [0.08, 0.1, 0.14, 1.0],
        );

        self.draw_filled_circle(
            vertices,
            indices,
            index_offset,
            self.sv_knob_position(picker, layout),
            5.5,
            [1.0, 1.0, 1.0, 1.0],
        );
        self.draw_smooth_circle_outline(
            vertices,
            indices,
            index_offset,
            self.sv_knob_position(picker, layout),
            6.0,
            1.5,
            [0.08, 0.1, 0.14, 1.0],
        );
    }

    fn generate_toolbar(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        current_tool: Tool,
        screen_size: (f32, f32),
    ) {
        let layout = UiLayout::new(screen_size);
        let icon_size = layout.toolbar_icon_size;
        let icon_spacing = layout.toolbar_spacing;
        let toolbar_padding = layout.toolbar_padding;
        let toolbar_width = layout.toolbar_size[0];
        let toolbar_height = layout.toolbar_size[1];
        let toolbar_center = layout.toolbar_center;

        let shadow_offset = 3.0;
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [
                toolbar_center[0] + shadow_offset,
                toolbar_center[1] + shadow_offset,
            ],
            [toolbar_width, toolbar_height],
            [0.0, 0.0, 0.0, 0.15],
            12.0 * layout.scale,
            0.0,
        );

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            toolbar_center,
            [toolbar_width, toolbar_height],
            [0.96, 0.96, 0.97, 0.98],
            12.0 * layout.scale,
            1.5,
        );

        let start_x = toolbar_center[0] - toolbar_width / 2.0 + toolbar_padding + icon_size / 2.0;

        for (i, icon) in self.tool_icons.iter().enumerate() {
            let x = start_x + i as f32 * (icon_size + icon_spacing);
            let y = toolbar_center[1];

            let is_selected = icon.tool == current_tool;

            let button_color = if is_selected {
                [0.25, 0.55, 0.95, 1.0]
            } else {
                [0.85, 0.85, 0.87, 1.0]
            };

            let luminance =
                0.299 * button_color[0] + 0.587 * button_color[1] + 0.114 * button_color[2];
            self.create_rounded_rect(
                vertices,
                indices,
                index_offset,
                [x, y],
                [icon_size, icon_size],
                button_color,
                8.0 * layout.scale,
                if is_selected { 0.0 } else { 1.5 },
            );

            let icon_color = if luminance < 0.5 {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.2, 0.2, 0.2, 1.0]
            };

            self.draw_tool_icon(
                vertices,
                indices,
                index_offset,
                icon.tool,
                [x, y],
                icon_size * 0.5,
                icon_color,
            );
        }
    }

    fn draw_plus_icon(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        center: [f32; 2],
        size: f32,
        color: [f32; 4],
    ) {
        self.create_simple_rect(vertices, indices, index_offset, center, [size, 2.0], color);
        self.create_simple_rect(vertices, indices, index_offset, center, [2.0, size], color);
    }

    fn generate_hue_slider(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        _picker: &ColorPickerState,
        layout: UiLayout,
    ) {
        let steps = 48;
        let x0 = layout.hue_origin[0];
        let x1 = layout.hue_origin[0] + layout.hue_size[0];
        for i in 0..steps {
            let t0 = i as f32 / steps as f32;
            let t1 = (i + 1) as f32 / steps as f32;
            let y0 = layout.hue_origin[1] + t0 * layout.hue_size[1];
            let y1 = layout.hue_origin[1] + t1 * layout.hue_size[1];
            let c0 = hsv_to_rgb((1.0 - t0) * 360.0, 1.0, 1.0);
            let c1 = hsv_to_rgb((1.0 - t1) * 360.0, 1.0, 1.0);
            self.create_colored_triangle(
                vertices,
                indices,
                index_offset,
                [x0, y0],
                c0,
                [x1, y0],
                c0,
                [x1, y1],
                c1,
            );
            self.create_colored_triangle(
                vertices,
                indices,
                index_offset,
                [x0, y0],
                c0,
                [x1, y1],
                c1,
                [x0, y1],
                c1,
            );
        }

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [
                layout.hue_origin[0] + layout.hue_size[0] * 0.5,
                layout.hue_origin[1] + layout.hue_size[1] * 0.5,
            ],
            layout.hue_size,
            [0.0, 0.0, 0.0, 0.0],
            12.0,
            1.0,
        );
    }

    fn generate_sv_panel(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        picker: &ColorPickerState,
        layout: UiLayout,
    ) {
        let steps = 20;
        let left = layout.sv_origin[0];
        let top = layout.sv_origin[1];

        for y in 0..steps {
            for x in 0..steps {
                let x0 = x as f32 / steps as f32;
                let x1 = (x + 1) as f32 / steps as f32;
                let y0 = y as f32 / steps as f32;
                let y1 = (y + 1) as f32 / steps as f32;

                let p00 = [left + x0 * layout.sv_size[0], top + y0 * layout.sv_size[1]];
                let p10 = [left + x1 * layout.sv_size[0], top + y0 * layout.sv_size[1]];
                let p11 = [left + x1 * layout.sv_size[0], top + y1 * layout.sv_size[1]];
                let p01 = [left + x0 * layout.sv_size[0], top + y1 * layout.sv_size[1]];

                let c00 = hsv_to_rgb(picker.hue, x0.clamp(0.0, 1.0), (1.0 - y0).clamp(0.0, 1.0));
                let c10 = hsv_to_rgb(picker.hue, x1.clamp(0.0, 1.0), (1.0 - y0).clamp(0.0, 1.0));
                let c11 = hsv_to_rgb(picker.hue, x1.clamp(0.0, 1.0), (1.0 - y1).clamp(0.0, 1.0));
                let c01 = hsv_to_rgb(picker.hue, x0.clamp(0.0, 1.0), (1.0 - y1).clamp(0.0, 1.0));

                self.create_colored_triangle(
                    vertices,
                    indices,
                    index_offset,
                    p00,
                    c00,
                    p10,
                    c10,
                    p11,
                    c11,
                );
                self.create_colored_triangle(
                    vertices,
                    indices,
                    index_offset,
                    p00,
                    c00,
                    p11,
                    c11,
                    p01,
                    c01,
                );
            }
        }

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [
                left + layout.sv_size[0] * 0.5,
                top + layout.sv_size[1] * 0.5,
            ],
            layout.sv_size,
            [0.0, 0.0, 0.0, 0.0],
            14.0,
            1.0,
        );
    }

    fn hue_knob_position(&self, picker: &ColorPickerState, layout: UiLayout) -> [f32; 2] {
        [
            layout.hue_origin[0] + layout.hue_size[0] * 0.5,
            layout.hue_origin[1] + (1.0 - picker.hue / 360.0) * layout.hue_size[1],
        ]
    }

    fn sv_knob_position(&self, picker: &ColorPickerState, layout: UiLayout) -> [f32; 2] {
        [
            layout.sv_origin[0] + picker.saturation * layout.sv_size[0],
            layout.sv_origin[1] + (1.0 - picker.value) * layout.sv_size[1],
        ]
    }

    fn color_from_picker(&self, picker: &ColorPickerState) -> [f32; 4] {
        hsv_to_rgb(picker.hue, picker.saturation, picker.value)
    }

    fn pick_hue_color(
        &self,
        mouse_pos: [f32; 2],
        picker: &ColorPickerState,
        layout: UiLayout,
    ) -> Option<[f32; 4]> {
        if !point_in_rect(mouse_pos, layout.hue_origin, layout.hue_size) {
            return None;
        }
        let hue = (1.0
            - ((mouse_pos[1] - layout.hue_origin[1]) / layout.hue_size[1]).clamp(0.0, 1.0))
            * 360.0;
        Some(hsv_to_rgb(hue, picker.saturation, picker.value))
    }

    fn pick_sv_color(
        &self,
        mouse_pos: [f32; 2],
        picker: &ColorPickerState,
        layout: UiLayout,
    ) -> Option<[f32; 4]> {
        if !point_in_rect(mouse_pos, layout.sv_origin, layout.sv_size) {
            return None;
        }

        let saturation = ((mouse_pos[0] - layout.sv_origin[0]) / layout.sv_size[0]).clamp(0.0, 1.0);
        let value =
            (1.0 - ((mouse_pos[1] - layout.sv_origin[1]) / layout.sv_size[1])).clamp(0.0, 1.0);
        Some(hsv_to_rgb(picker.hue, saturation, value))
    }

    fn generate_zoom_indicator(
        &self,
        vertices: &mut Vec<UiVertex>,
        indices: &mut Vec<u16>,
        index_offset: &mut u16,
        screen_size: (f32, f32),
    ) {
        let layout = UiLayout::new(screen_size);
        let zoom_size = layout.zoom_size();
        let zoom_bg_center = layout.zoom_center();

        let shadow_offset = 2.0;
        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            [
                zoom_bg_center[0] + shadow_offset,
                zoom_bg_center[1] + shadow_offset,
            ],
            zoom_size,
            [0.0, 0.0, 0.0, 0.1],
            8.0 * layout.scale,
            0.0,
        );

        self.create_rounded_rect(
            vertices,
            indices,
            index_offset,
            zoom_bg_center,
            zoom_size,
            [0.2, 0.2, 0.2, 0.9],
            8.0 * layout.scale,
            1.0,
        );
    }

    pub fn zoom_label_layout(&self, screen_size: (f32, f32)) -> ([f32; 2], f32) {
        let layout = UiLayout::new(screen_size);
        (layout.zoom_text_pos(), layout.zoom_font_size())
    }

    pub fn generate_ui_vertices(
        &self,
        current_tool: Tool,
        current_color: [f32; 4],
        picker: &ColorPickerState,
        screen_size: (f32, f32),
        _zoom_level: f32,
    ) -> (Vec<UiVertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        self.generate_color_picker(
            &mut vertices,
            &mut indices,
            &mut index_offset,
            current_color,
            picker,
            screen_size,
        );
        self.generate_toolbar(
            &mut vertices,
            &mut indices,
            &mut index_offset,
            current_tool,
            screen_size,
        );
        self.generate_zoom_indicator(&mut vertices, &mut indices, &mut index_offset, screen_size);

        (vertices, indices)
    }

    pub fn handle_color_interaction(
        &self,
        mouse_pos: [f32; 2],
        picker: &ColorPickerState,
        screen_size: (f32, f32),
    ) -> ColorInteraction {
        let layout = UiLayout::new(screen_size);
        for (i, &color) in PALETTE_COLORS.iter().enumerate() {
            let col = (i % 2) as f32;
            let row = (i / 2) as f32;

            let x = layout.palette_origin[0] + col * (layout.swatch_size + layout.swatch_padding);
            let y = layout.palette_origin[1] + row * (layout.swatch_size + layout.swatch_padding);

            if mouse_pos[0] >= x
                && mouse_pos[0] <= x + layout.swatch_size
                && mouse_pos[1] >= y
                && mouse_pos[1] <= y + layout.swatch_size
            {
                return ColorInteraction::Color(color);
            }
        }

        let custom_center = layout.custom_swatch_center();
        if point_in_rect(
            mouse_pos,
            [
                custom_center[0] - layout.swatch_size * 0.5,
                custom_center[1] - layout.swatch_size * 0.5,
            ],
            [layout.swatch_size, layout.swatch_size],
        ) {
            return ColorInteraction::TogglePicker;
        }

        if !picker.open {
            return ColorInteraction::None;
        }

        if let Some(color) = self.pick_sv_color(mouse_pos, picker, layout) {
            return ColorInteraction::BeginDrag(ColorPickerDragMode::SvDisk, color);
        }

        if let Some(color) = self.pick_hue_color(mouse_pos, picker, layout) {
            return ColorInteraction::BeginDrag(ColorPickerDragMode::HueRing, color);
        }

        ColorInteraction::None
    }

    pub fn handle_color_drag(
        &self,
        mouse_pos: [f32; 2],
        picker: &ColorPickerState,
        drag_mode: ColorPickerDragMode,
        screen_size: (f32, f32),
    ) -> Option<[f32; 4]> {
        let layout = UiLayout::new(screen_size);
        match drag_mode {
            ColorPickerDragMode::HueRing => self.pick_hue_color(mouse_pos, picker, layout),
            ColorPickerDragMode::SvDisk => self.pick_sv_color(mouse_pos, picker, layout),
        }
    }

    pub fn handle_click(&self, mouse_pos: [f32; 2], screen_size: (f32, f32)) -> Option<Tool> {
        let layout = UiLayout::new(screen_size);
        let icon_size = layout.toolbar_icon_size;
        let icon_spacing = layout.toolbar_spacing;
        let toolbar_padding = layout.toolbar_padding;
        let toolbar_width = layout.toolbar_size[0];
        let toolbar_height = layout.toolbar_size[1];
        let toolbar_center = layout.toolbar_center;

        let toolbar_left = toolbar_center[0] - toolbar_width / 2.0;
        let toolbar_right = toolbar_center[0] + toolbar_width / 2.0;
        let toolbar_top = toolbar_center[1] - toolbar_height / 2.0;
        let toolbar_bottom = toolbar_center[1] + toolbar_height / 2.0;

        if mouse_pos[0] < toolbar_left
            || mouse_pos[0] > toolbar_right
            || mouse_pos[1] < toolbar_top
            || mouse_pos[1] > toolbar_bottom
        {
            return None;
        }

        let start_x = toolbar_center[0] - toolbar_width / 2.0 + toolbar_padding;

        for (i, icon) in self.tool_icons.iter().enumerate() {
            let button_left = start_x + i as f32 * (icon_size + icon_spacing);
            let button_right = button_left + icon_size;
            let button_top = toolbar_center[1] - icon_size / 2.0;
            let button_bottom = toolbar_center[1] + icon_size / 2.0;

            if mouse_pos[0] >= button_left
                && mouse_pos[0] <= button_right
                && mouse_pos[1] >= button_top
                && mouse_pos[1] <= button_bottom
            {
                return Some(icon.tool);
            }
        }

        None
    }

    pub fn is_mouse_over_ui(
        &self,
        mouse_pos: [f32; 2],
        screen_size: (f32, f32),
        picker: &ColorPickerState,
    ) -> bool {
        let layout = UiLayout::new(screen_size);
        let toolbar_width = layout.toolbar_size[0];
        let toolbar_height = layout.toolbar_size[1];
        let toolbar_center = layout.toolbar_center;

        let toolbar_left = toolbar_center[0] - toolbar_width / 2.0;
        let toolbar_right = toolbar_center[0] + toolbar_width / 2.0;
        let toolbar_top = toolbar_center[1] - toolbar_height / 2.0;
        let toolbar_bottom = toolbar_center[1] + toolbar_height / 2.0;

        if mouse_pos[0] >= toolbar_left
            && mouse_pos[0] <= toolbar_right
            && mouse_pos[1] >= toolbar_top
            && mouse_pos[1] <= toolbar_bottom
        {
            return true;
        }

        let padding = layout.swatch_padding;
        let start_x = layout.palette_origin[0];
        let start_y = layout.palette_origin[1];
        let swatch_size = layout.swatch_size;
        let cols = 2;
        let rows = (PALETTE_COLORS.len() + cols) / cols + 1;

        let palette_width = cols as f32 * swatch_size + (cols - 1) as f32 * padding;
        let palette_height = rows as f32 * swatch_size + (rows - 1) as f32 * padding;

        let over_palette = mouse_pos[0] >= start_x
            && mouse_pos[0] <= start_x + palette_width
            && mouse_pos[1] >= start_y
            && mouse_pos[1] <= start_y + palette_height;

        if over_palette {
            return true;
        }

        picker.open && picker_bounds_contains(mouse_pos, layout)
    }

    pub fn generate_toolbar_icons(
        &self,
        text_renderer: &mut crate::text_renderer::TextRenderer,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        current_tool: Tool,
        screen_size: (f32, f32),
    ) {
        let icon_size = 36.0;
        let icon_spacing = 12.0;
        let toolbar_padding = 20.0;
        let num_icons = self.tool_icons.len() as f32;

        #[cfg(target_os = "macos")]
        let titlebar_padding = 1.0;
        #[cfg(not(target_os = "macos"))]
        let titlebar_padding = 0.0;

        let toolbar_width =
            num_icons * icon_size + (num_icons - 1.0) * icon_spacing + 2.0 * toolbar_padding;
        let toolbar_center = [
            screen_size.0 / 2.0,
            titlebar_padding + toolbar_padding + (icon_size + 2.0 * toolbar_padding) / 2.0,
        ];
        let start_x = toolbar_center[0] - toolbar_width / 2.0 + toolbar_padding + icon_size / 2.0;

        for (i, icon) in self.tool_icons.iter().enumerate() {
            let x = start_x + i as f32 * (icon_size + icon_spacing);
            let y = toolbar_center[1];

            let is_selected = icon.tool == current_tool;

            let button_color = if is_selected {
                [0.25, 0.55, 0.95, 1.0]
            } else {
                [0.85, 0.85, 0.87, 1.0]
            };

            let luminance =
                0.299 * button_color[0] + 0.587 * button_color[1] + 0.114 * button_color[2];
            let text_color = if luminance < 0.5 {
                [1.0, 1.0, 1.0, 1.0]
            } else {
                [0.2, 0.2, 0.2, 1.0]
            };

            let icon_text = match icon.tool {
                Tool::Select => "↖",
                Tool::Pen => "✎",
                Tool::Rectangle => "▭",
                Tool::Circle => "○",
                Tool::Diamond => "◇",
                Tool::Arrow => "→",
                Tool::Text => "T",
                Tool::Eraser => "⌫",
                Tool::Line => "|",
            };

            let text_pos = [x - 8.0, y + 6.0];

            text_renderer.add_screen_label(device, queue, icon_text, text_pos, 16.0, text_color);
        }
    }
}

fn point_in_rect(point: [f32; 2], origin: [f32; 2], size: [f32; 2]) -> bool {
    point[0] >= origin[0]
        && point[0] <= origin[0] + size[0]
        && point[1] >= origin[1]
        && point[1] <= origin[1] + size[1]
}

fn titlebar_padding() -> f32 {
    #[cfg(target_os = "macos")]
    {
        10.0
    }
    #[cfg(not(target_os = "macos"))]
    {
        0.0
    }
}

fn picker_bounds_contains(mouse_pos: [f32; 2], layout: UiLayout) -> bool {
    point_in_rect(
        mouse_pos,
        [layout.picker_origin[0], layout.picker_origin[1]],
        layout.picker_size,
    )
}

fn hsv_to_rgb(h: f32, s: f32, v: f32) -> [f32; 4] {
    let c = v * s;
    let hue_sector = (h / 60.0).rem_euclid(6.0);
    let x = c * (1.0 - ((hue_sector.rem_euclid(2.0)) - 1.0).abs());
    let (r1, g1, b1) = if hue_sector < 1.0 {
        (c, x, 0.0)
    } else if hue_sector < 2.0 {
        (x, c, 0.0)
    } else if hue_sector < 3.0 {
        (0.0, c, x)
    } else if hue_sector < 4.0 {
        (0.0, x, c)
    } else if hue_sector < 5.0 {
        (x, 0.0, c)
    } else {
        (c, 0.0, x)
    };
    let m = v - c;
    [r1 + m, g1 + m, b1 + m, 1.0]
}
