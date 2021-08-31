use na::{Mat4, Vec4};
use std::f32::consts::PI;

use crate::types::Vector3;

/// 90 degrees in radians
const R89: f32 = (PI / 180.0) * 89.0;

/// 180 degrees in radians
const R180: f32 = PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Transform {
    /// Position of the object
    pub position: Vector3,

    /// Rotation of the object (euler angles in radians)
    pub rotation: Vector3,
}

impl Transform {
    pub fn rotate(&mut self, vec: Vector3) {
        self.rotation += vec;

        // Clamp -pi/2 < pitch < pi/2
        if self.rotation.x > R89 {
            self.rotation.x = R89;
        } else if self.rotation.x <= -R89 {
            self.rotation.x = -R89;
        }

        // -pi < yaw <= pi
        if self.rotation.y <= -R180 {
            self.rotation.y = R180 - self.rotation.y % -R180;
        } else if self.rotation.y > 180.0 {
            self.rotation.y = -R180 + self.rotation.y % R180;
        }
    }

    pub fn translate(&mut self, delta: Vector3) {
        let rot_matrix =
            Mat4::from_euler_angles(-self.rotation.x, self.rotation.y, self.rotation.z);

        let new = rot_matrix * Vec4::new(delta.x, delta.y, delta.z, 1.0);
        self.position.x += new.x;
        self.position.y += new.y;
        self.position.z += new.z;
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct CameraSettings {
    /// FOV (radians)
    pub fov: f32,

    /// Near clipping plane (world units)
    pub near: f32,

    /// Far clipping plane (world units)
    pub far: f32,
}
