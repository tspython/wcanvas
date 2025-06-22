mod vec3;
mod mat4;

pub use vec3::Vec3;
pub use mat4::Mat4;

pub const PI: f32 = std::f32::consts::PI;
pub const TAU: f32 = std::f32::consts::TAU;

pub fn ortho(left: f32, right: f32, bottom: f32, top: f32, near: f32, far: f32) -> Mat4 {
    let w = right - left;
    let h = top - bottom;
    let d = far - near;
    
    let result = Mat4::new([
        [2.0 / w,    0.0,        0.0,         0.0],
        [0.0,        2.0 / h,    0.0,         0.0],
        [0.0,        0.0,        -2.0 / d,    0.0],
        [-(right + left) / w, -(top + bottom) / h, -(far + near) / d, 1.0],
    ]);
    
    log::info!("Ortho matrix: left={}, right={}, bottom={}, top={}, near={}, far={}", left, right, bottom, top, near, far);
    log::info!("Matrix: {:?}", result);
    
    result
}

pub fn clamp(value: f32, min: f32, max: f32) -> f32 {
    if value < min {
        min
    } else if value > max {
        max
    } else {
        value
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_identity_matrix() {
        let identity = Mat4::identity();
        let test_vec = Vec3::new(1.0, 2.0, 3.0);
        let result = identity.transform_point(test_vec);
        
        assert!((result.x - test_vec.x).abs() < 0.001);
        assert!((result.y - test_vec.y).abs() < 0.001);
        assert!((result.z - test_vec.z).abs() < 0.001);
    }
    
    #[test]
    fn test_translation_matrix() {
        let translation = Mat4::from_translation(Vec3::new(10.0, 20.0, 30.0));
        let test_vec = Vec3::new(1.0, 2.0, 3.0);
        let result = translation.transform_point(test_vec);
        
        assert!((result.x - 11.0).abs() < 0.001);
        assert!((result.y - 22.0).abs() < 0.001);
        assert!((result.z - 33.0).abs() < 0.001);
    }
    
    #[test]
    fn test_matrix_multiplication() {
        let identity = Mat4::identity();
        let translation = Mat4::from_translation(Vec3::new(1.0, 2.0, 3.0));
        let result = identity * translation;
        
        assert_eq!(result, translation);
    }
} 