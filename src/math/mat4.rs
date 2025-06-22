use crate::math::Vec3;

#[repr(C)]
#[derive(Debug, Copy, Clone, PartialEq)]
pub struct Mat4 {
    pub data: [[f32; 4]; 4],
}

impl Mat4 {
    pub fn new(data: [[f32; 4]; 4]) -> Self {
        Self { data }
    }

    pub fn identity() -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn zero() -> Self {
        Self {
            data: [
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
                [0.0, 0.0, 0.0, 0.0],
            ],
        }
    }

    pub fn from_translation(translation: Vec3) -> Self {
        Self {
            data: [
                [1.0, 0.0, 0.0, 0.0],
                [0.0, 1.0, 0.0, 0.0],
                [0.0, 0.0, 1.0, 0.0],
                [translation.x, translation.y, translation.z, 1.0],
            ],
        }
    }

    pub fn from_scale(scale: f32) -> Self {
        Self {
            data: [
                [scale, 0.0, 0.0, 0.0],
                [0.0, scale, 0.0, 0.0],
                [0.0, 0.0, scale, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn from_nonuniform_scale(x: f32, y: f32, z: f32) -> Self {
        Self {
            data: [
                [x, 0.0, 0.0, 0.0],
                [0.0, y, 0.0, 0.0],
                [0.0, 0.0, z, 0.0],
                [0.0, 0.0, 0.0, 1.0],
            ],
        }
    }

    pub fn transpose(self) -> Self {
        let mut result = Self::zero();
        for i in 0..4 {
            for j in 0..4 {
                result.data[i][j] = self.data[j][i];
            }
        }
        result
    }

    pub fn column(&self, index: usize) -> [f32; 4] {
        [
            self.data[index][0],
            self.data[index][1],
            self.data[index][2],
            self.data[index][3],
        ]
    }

    pub fn transform_point(&self, point: Vec3) -> Vec3 {
        let x = self.data[0][0] * point.x
            + self.data[1][0] * point.y
            + self.data[2][0] * point.z
            + self.data[3][0];
        let y = self.data[0][1] * point.x
            + self.data[1][1] * point.y
            + self.data[2][1] * point.z
            + self.data[3][1];
        let z = self.data[0][2] * point.x
            + self.data[1][2] * point.y
            + self.data[2][2] * point.z
            + self.data[3][2];
        let w = self.data[0][3] * point.x
            + self.data[1][3] * point.y
            + self.data[2][3] * point.z
            + self.data[3][3];

        if w != 0.0 {
            Vec3::new(x / w, y / w, z / w)
        } else {
            Vec3::new(x, y, z)
        }
    }

    pub fn transform_vector(&self, vector: Vec3) -> Vec3 {
        let x =
            self.data[0][0] * vector.x + self.data[1][0] * vector.y + self.data[2][0] * vector.z;
        let y =
            self.data[0][1] * vector.x + self.data[1][1] * vector.y + self.data[2][1] * vector.z;
        let z =
            self.data[0][2] * vector.x + self.data[1][2] * vector.y + self.data[2][2] * vector.z;

        Vec3::new(x, y, z)
    }
}

impl std::ops::Mul for Mat4 {
    type Output = Self;

    fn mul(self, other: Self) -> Self {
        let mut result = Self::zero();

        for i in 0..4 {
            for j in 0..4 {
                for k in 0..4 {
                    result.data[i][j] += self.data[k][j] * other.data[i][k];
                }
            }
        }

        result
    }
}

impl From<Mat4> for [[f32; 4]; 4] {
    fn from(mat: Mat4) -> Self {
        mat.data
    }
}

impl From<[[f32; 4]; 4]> for Mat4 {
    fn from(data: [[f32; 4]; 4]) -> Self {
        Self { data }
    }
}

unsafe impl bytemuck::Pod for Mat4 {}
unsafe impl bytemuck::Zeroable for Mat4 {}
