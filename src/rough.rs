use crate::Vertex;
use rand::Rng;
use rand::SeedableRng;
use rand::rngs::StdRng;

#[derive(Debug, Clone)]
pub struct RoughOptions {
    pub roughness: f32,           
    pub bowing: f32,              
    pub stroke_width: f32,        
    pub max_randomness_offset: f32, 
    pub curve_step_count: u32,    
    pub disable_multi_stroke: bool, 
    pub seed: Option<u64>,        
    pub curve_tightness: f32,     
    pub preserve_vertices: bool,  
}

impl Default for RoughOptions {
    fn default() -> Self {
        Self {
            roughness: 1.0,
            bowing: 1.0,
            stroke_width: 1.0,
            max_randomness_offset: 2.0,
            curve_step_count: 32,
            disable_multi_stroke: false,
            seed: None,
            curve_tightness: 0.0,
            preserve_vertices: false,
        }
    }
}

pub struct RoughGenerator {
    rng: StdRng,
}

impl RoughGenerator {
    pub fn new(seed: Option<u64>) -> Self {
        let rng = if let Some(seed) = seed {
            StdRng::seed_from_u64(seed)
        } else {
            StdRng::seed_from_u64(42)
        };
        Self { rng }
    }

    fn random(&mut self) -> f32 {
        self.rng.random::<f32>()
    }

    fn offset(&mut self, min: f32, max: f32, options: &RoughOptions, roughness_gain: f32) -> f32 {
        options.roughness * roughness_gain * ((self.random() * (max - min)) + min)
    }

    fn offset_opt(&mut self, x: f32, options: &RoughOptions, roughness_gain: f32) -> f32 {
        self.offset(-x, x, options, roughness_gain)
    }

    pub fn rough_line(&mut self, start: [f32; 2], end: [f32; 2], options: &RoughOptions) -> Vec<[f32; 2]> {
        let mut points = Vec::new();
        
        let length_sq = (start[0] - end[0]).powi(2) + (start[1] - end[1]).powi(2);
        let length = length_sq.sqrt();
        
        let mut roughness_gain = 1.0;
        if length < 200.0 {
            roughness_gain = 1.0;
        } else if length > 500.0 {
            roughness_gain = 0.4;
        } else {
            roughness_gain = (-0.0016668) * length + 1.233334;
        }

        let mut offset = options.max_randomness_offset;
        if (offset * offset * 100.0) > length_sq {
            offset = length / 10.0;
        }

        let diverge_point = 0.2 + self.random() * 0.2;
        let mid_disp_x = options.bowing * options.max_randomness_offset * (end[1] - start[1]) / 200.0;
        let mid_disp_y = options.bowing * options.max_randomness_offset * (start[0] - end[0]) / 200.0;

        let mid_disp_x = mid_disp_x + self.offset_opt(mid_disp_x, options, roughness_gain);
        let mid_disp_y = mid_disp_y + self.offset_opt(mid_disp_y, options, roughness_gain);

        let start_x_offset = if options.preserve_vertices { 0.0 } else { self.offset_opt(offset, options, roughness_gain) };
        let start_y_offset = if options.preserve_vertices { 0.0 } else { self.offset_opt(offset, options, roughness_gain) };
        points.push([start[0] + start_x_offset, start[1] + start_y_offset]);

        let cp1 = [
            mid_disp_x + start[0] + (end[0] - start[0]) * diverge_point + self.offset_opt(offset, options, roughness_gain),
            mid_disp_y + start[1] + (end[1] - start[1]) * diverge_point + self.offset_opt(offset, options, roughness_gain),
        ];
        let cp2 = [
            mid_disp_x + start[0] + 2.0 * (end[0] - start[0]) * diverge_point + self.offset_opt(offset, options, roughness_gain),
            mid_disp_y + start[1] + 2.0 * (end[1] - start[1]) * diverge_point + self.offset_opt(offset, options, roughness_gain),
        ];

        let end_x_offset = if options.preserve_vertices { 0.0 } else { self.offset_opt(offset, options, roughness_gain) };
        let end_y_offset = if options.preserve_vertices { 0.0 } else { self.offset_opt(offset, options, roughness_gain) };
        let bezier_points = self.bezier_curve(points[0], cp1, cp2, [
            end[0] + end_x_offset,
            end[1] + end_y_offset
        ], 10);

        points.extend(bezier_points);
        points
    }

    fn bezier_curve(&self, p0: [f32; 2], p1: [f32; 2], p2: [f32; 2], p3: [f32; 2], segments: u32) -> Vec<[f32; 2]> {
        let mut points = Vec::new();
        
        for i in 1..=segments {
            let t = i as f32 / segments as f32;
            let u = 1.0 - t;
            let tt = t * t;
            let uu = u * u;
            let uuu = uu * u;
            let ttt = tt * t;

            let x = uuu * p0[0] + 3.0 * uu * t * p1[0] + 3.0 * u * tt * p2[0] + ttt * p3[0];
            let y = uuu * p0[1] + 3.0 * uu * t * p1[1] + 3.0 * u * tt * p2[1] + ttt * p3[1];

            points.push([x, y]);
        }
        
        points
    }

    pub fn rough_rectangle(&mut self, position: [f32; 2], size: [f32; 2], options: &RoughOptions) -> Vec<Vec<[f32; 2]>> {
        let corners = [
            position,
            [position[0] + size[0], position[1]],
            [position[0] + size[0], position[1] + size[1]],
            [position[0], position[1] + size[1]],
        ];

        let mut lines = Vec::new();
        
        for i in 0..4 {
            let start = corners[i];
            let end = corners[(i + 1) % 4];
            
            let line = self.rough_line(start, end, options);
            lines.push(line);
            
            if !options.disable_multi_stroke {
                let line2 = self.rough_line(start, end, options);
                lines.push(line2);
            }
        }

        lines
    }

    pub fn rough_ellipse(&mut self, center: [f32; 2], width: f32, height: f32, options: &RoughOptions) -> Vec<Vec<[f32; 2]>> {
        let rx = width / 2.0;
        let ry = height / 2.0;
        
        let base_step_count = options.curve_step_count;
        let step_variation = (self.random() * 4.0) as u32;
        let step_count = (base_step_count + step_variation).max(16).min(48);
        let increment = (std::f32::consts::PI * 2.0) / step_count as f32;

        let rx_offset = rx + self.offset_opt(rx * 0.02, options, 1.0);
        let ry_offset = ry + self.offset_opt(ry * 0.02, options, 1.0);

        let overlap = increment * self.offset(0.05, 0.1, options, 1.0);
        let points = self.compute_ellipse_points_varied(increment, center, rx_offset, ry_offset, 1.0, overlap, options, step_count);

        let mut result = vec![points];

        if !options.disable_multi_stroke {
            let stroke2_options = RoughOptions {
                roughness: options.roughness * 0.8,
                ..options.clone()
            };
            
            let rx_offset2 = rx + self.offset_opt(rx * 0.01, &stroke2_options, 1.0);
            let ry_offset2 = ry + self.offset_opt(ry * 0.01, &stroke2_options, 1.0);
            let overlap2 = increment * self.offset(0.02, 0.05, &stroke2_options, 1.0);
            let points2 = self.compute_ellipse_points_varied(increment, center, rx_offset2, ry_offset2, 0.5, overlap2, &stroke2_options, step_count);
            
            result.push(points2);
        }

        result
    }

    fn compute_ellipse_points_varied(&mut self, increment: f32, center: [f32; 2], rx: f32, ry: f32, offset: f32, overlap: f32, options: &RoughOptions, step_count: u32) -> Vec<[f32; 2]> {
        let mut points = Vec::new();
        
        if options.roughness == 0.0 {
            let mut angle = -increment;
            while angle <= std::f32::consts::PI * 2.0 {
                let radius_var_x = rx + self.offset_opt(0.2, options, 0.05);
                let radius_var_y = ry + self.offset_opt(0.2, options, 0.05);
                points.push([
                    center[0] + radius_var_x * angle.cos(),
                    center[1] + radius_var_y * angle.sin(),
                ]);
                angle += increment;
            }
        } else {
            let rad_offset = self.offset_opt(0.1, options, 1.0) - (std::f32::consts::PI / 2.0);
            
            let start_radius_variation = 0.98 + self.random() * 0.04;
            points.push([
                self.offset_opt(offset * 0.3, options, 1.0) + center[0] + start_radius_variation * rx * (rad_offset - increment).cos(),
                self.offset_opt(offset * 0.3, options, 1.0) + center[1] + start_radius_variation * ry * (rad_offset - increment).sin(),
            ]);

            let end_angle = std::f32::consts::PI * 2.0 + rad_offset + overlap;
            let mut angle = rad_offset;
            let mut segment_idx = 0;
            
            while angle < end_angle {
                let segment_progress = segment_idx as f32 / step_count as f32;
                
                let wave1 = (segment_progress * std::f32::consts::PI * 3.0).sin() * 0.01;
                let wave2 = (segment_progress * std::f32::consts::PI * 5.0).cos() * 0.005;
                let radius_modifier = 1.0 + wave1 + wave2 + self.offset_opt(0.02, options, 1.0);
                
                let radius_modifier = radius_modifier.max(0.95).min(1.05);
                
                let point_rx = rx * radius_modifier + self.offset_opt(rx * 0.01, options, 1.0);
                let point_ry = ry * radius_modifier + self.offset_opt(ry * 0.01, options, 1.0);
                
                let point_rx = point_rx.max(rx * 0.92).min(rx * 1.08);
                let point_ry = point_ry.max(ry * 0.92).min(ry * 1.08);
                
                points.push([
                    self.offset_opt(offset * 0.2, options, 1.0) + center[0] + point_rx * angle.cos(),
                    self.offset_opt(offset * 0.2, options, 1.0) + center[1] + point_ry * angle.sin(),
                ]);
                
                let increment_variation = increment * (0.95 + self.random() * 0.1);
                angle += increment_variation;
                segment_idx += 1;
            }

            let end_radius_variation = 0.96 + self.random() * 0.08;
            points.push([
                self.offset_opt(offset * 0.5, options, 1.0) + center[0] + end_radius_variation * rx * (rad_offset + std::f32::consts::PI * 2.0 + overlap * 0.5).cos(),
                self.offset_opt(offset * 0.5, options, 1.0) + center[1] + end_radius_variation * ry * (rad_offset + std::f32::consts::PI * 2.0 + overlap * 0.5).sin(),
            ]);
            
            let closure_variation = 0.95 + self.random() * 0.1;
            points.push([
                self.offset_opt(offset * 0.3, options, 1.0) + center[0] + closure_variation * rx * (rad_offset + overlap).cos(),
                self.offset_opt(offset * 0.3, options, 1.0) + center[1] + closure_variation * ry * (rad_offset + overlap).sin(),
            ]);
        }

        points
    }

    fn curve_through_points(&mut self, points: Vec<[f32; 2]>, close: bool, options: &RoughOptions) -> Vec<[f32; 2]> {
        if points.len() < 2 {
            return points;
        }

        let mut curve_points = Vec::new();
        let mut extended_points = Vec::new();

        extended_points.push(points[0]);
        extended_points.push(points[0]);
        extended_points.extend(points.iter().cloned());
        if points.len() > 1 {
            extended_points.push(points[points.len() - 1]);
        }

        if extended_points.len() > 3 {
            let s = 1.0 - options.curve_tightness;
            curve_points.push(extended_points[1]);

            for i in 1..extended_points.len() - 2 {
                let p0 = extended_points[i - 1];
                let p1 = extended_points[i];
                let p2 = extended_points[i + 1];
                let p3 = extended_points[i + 2];

                let cp1 = [
                    p1[0] + (s * p2[0] - s * p0[0]) / 6.0,
                    p1[1] + (s * p2[1] - s * p0[1]) / 6.0,
                ];
                let cp2 = [
                    p2[0] + (s * p1[0] - s * p3[0]) / 6.0,
                    p2[1] + (s * p1[1] - s * p3[1]) / 6.0,
                ];

                let bezier_points = self.bezier_curve(p1, cp1, cp2, p2, 8);
                curve_points.extend(bezier_points);
            }

            if close && points.len() > 2 {
                curve_points.push(extended_points[1]);
            }
        } else {
            curve_points = points;
        }

        curve_points
    }

    pub fn points_to_vertices(&self, points: &[[f32; 2]], color: [f32; 4], width: f32) -> (Vec<Vertex>, Vec<u16>) {
        let mut vertices = Vec::new();
        let mut indices = Vec::new();
        let mut index_offset = 0u16;

        for i in 0..points.len().saturating_sub(1) {
            let p1 = points[i];
            let p2 = points[i + 1];

            let dx = p2[0] - p1[0];
            let dy = p2[1] - p1[1];
            let len = (dx * dx + dy * dy).sqrt();
            
            if len > 0.0 {
                let nx = -dy / len * width * 0.5;
                let ny = dx / len * width * 0.5;

                vertices.extend_from_slice(&[
                    Vertex { position: [p1[0] - nx, p1[1] - ny], color },
                    Vertex { position: [p1[0] + nx, p1[1] + ny], color },
                    Vertex { position: [p2[0] + nx, p2[1] + ny], color },
                    Vertex { position: [p2[0] - nx, p2[1] - ny], color },
                ]);

                indices.extend_from_slice(&[
                    index_offset, index_offset + 1, index_offset + 2,
                    index_offset, index_offset + 2, index_offset + 3,
                ]);
                index_offset += 4;
            }
        }

        (vertices, indices)
    }

    pub fn rough_arrow(&mut self, start: [f32; 2], end: [f32; 2], options: &RoughOptions) -> Vec<Vec<[f32; 2]>> {
        let mut lines = Vec::new();
        
        let shaft_line = self.rough_line(start, end, options);
        lines.push(shaft_line);
        
        if !options.disable_multi_stroke {
            let shaft_line2 = self.rough_line(start, end, options);
            lines.push(shaft_line2);
        }
        
        let dx = end[0] - start[0];
        let dy = end[1] - start[1];
        let len = (dx * dx + dy * dy).sqrt();
        
        if len > 0.0 {
            let head_len = (20.0 + self.offset_opt(5.0, options, 1.0)).max(10.0);
            let head_angle = 0.5 + self.offset_opt(0.1, options, 1.0);
            
            let dir_x = dx / len;
            let dir_y = dy / len;
            
            let cos_angle = head_angle.cos();
            let sin_angle = head_angle.sin();
            
            let left_x = end[0] - head_len * (dir_x * cos_angle - dir_y * sin_angle);
            let left_y = end[1] - head_len * (dir_y * cos_angle + dir_x * sin_angle);
            
            let right_x = end[0] - head_len * (dir_x * cos_angle + dir_y * sin_angle);
            let right_y = end[1] - head_len * (dir_y * cos_angle - dir_x * sin_angle);
            
            let left_line = self.rough_line([left_x, left_y], end, options);
            let right_line = self.rough_line([right_x, right_y], end, options);
            
            lines.push(left_line);
            lines.push(right_line);
            
            if !options.disable_multi_stroke {
                let left_line2 = self.rough_line([left_x, left_y], end, options);
                let right_line2 = self.rough_line([right_x, right_y], end, options);
                lines.push(left_line2);
                lines.push(right_line2);
            }
        }
        
        lines
    }
}
