// Copyright (C) 2019 Oscar Shrimpton

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Things related to converting 3D world space to 2D screen space

use stockton_types::{Vector3, Matrix4};

use std::f32::consts::PI;
use na::{look_at_lh, perspective_lh_zo, Mat4, Vec4};

/// 90 degrees in radians
const R89: f32 = (PI / 180.0) * 89.0;

/// 90 degrees in radians
const R90: f32 = PI / 2.0;

/// 180 degrees in radians
const R180: f32 = PI;

fn euler_to_direction(euler: &Vector3) -> Vector3 {
	let pitch = euler.x;
	let yaw = euler.y;
	let _roll = euler.z; // TODO: Support camera roll

	Vector3::new(
		yaw.sin() * pitch.cos(),
		pitch.sin(),
		yaw.cos() * pitch.cos()
	)
}

pub struct CameraSettings {
	/// Position of the camera (world units)
	pub position: Vector3,

	/// Rotation of the camera (euler angles in radians)
	pub rotation: Vector3,

	/// The up direction (normalized)
	pub up: Vector3,

	/// FOV (radians)
	pub fov: f32,

	/// Near clipping plane (world units)
	pub near: f32,

	/// Far clipping plane (world units)
	pub far: f32,
}

/// Holds settings related to the projection of world space to screen space
/// Also holds maths for generating important matrices
pub struct WorkingCamera {
	/// Settings for the camera
	settings: CameraSettings,

	/// Aspect ratio as a fraction
	aspect_ratio: f32,

	/// Cached view projection matrix
	vp_matrix: Mat4,

	/// If true, cached value needs updated
	is_dirty: bool
}

impl WorkingCamera {
	/// Return a camera with default settings
	pub fn defaults(aspect_ratio: f32) -> WorkingCamera {
		WorkingCamera::with_settings(CameraSettings {
			position: Vector3::new(0.0, 0.0, 0.0),
			rotation: Vector3::new(0.0, R90, 0.0),
			up: Vector3::new(0.0, 1.0, 0.0),
			fov: f32::to_radians(90.0),
			near: 0.1,
			far: 1024.0,
		}, aspect_ratio)
	}

	/// Return a camera with the given settings
	pub fn with_settings(settings: CameraSettings, aspect_ratio: f32) -> WorkingCamera {
		WorkingCamera {
			aspect_ratio,
			settings,
			vp_matrix: Mat4::identity(),
			is_dirty: true
		}
	}

	/// Get the VP matrix, updating cache if needed
	pub fn get_matrix<'a>(&'a mut self) -> &'a Mat4 {
		// Update matrix if needed
		if self.is_dirty {
			self.vp_matrix = self.calc_vp_matrix();
			self.is_dirty = false;
		}

		// Return the matrix
		&self.vp_matrix
	}

	/// Returns a matrix that transforms from world space to screen space
	fn calc_vp_matrix(&self) -> Matrix4 {
		// Get look direction from euler angles
		let direction = euler_to_direction(&self.settings.rotation);

		// Converts world space to camera space
		let view_matrix = look_at_lh(
			&self.settings.position,
			&(direction + &self.settings.position),
			&self.settings.up
		);

		// Converts camera space to screen space
		let projection_matrix = {
			let mut temp = perspective_lh_zo(
				self.aspect_ratio,
				self.settings.fov,
				self.settings.near,
				self.settings.far
			);

			// Vulkan's co-ord system is different from OpenGLs
			temp[(1, 1)] *= -1.0;

			temp
		};

		// Chain them together into a single matrix
		projection_matrix * view_matrix
	}

	/// Update the aspect ratio
	pub fn update_aspect_ratio(&mut self, new: f32) {
		self.aspect_ratio = new;
		self.is_dirty = true;
	}

	/// Apply rotation of the camera
	/// `euler` should be euler angles in degrees
	pub fn rotate(&mut self, euler: Vector3) {
		// TODO
		self.settings.rotation += euler;

		// Clamp -pi/2 < pitch < pi/2
		if self.settings.rotation.x > R89 {
			self.settings.rotation.x = R89;
		} else if self.settings.rotation.x <= -R89 {
			self.settings.rotation.x = -R89;
		}

		// -pi < yaw <= pi
		if self.settings.rotation.y <= -R180 {
			self.settings.rotation.y = R180 - self.settings.rotation.y % -R180;
		} else if self.settings.rotation.y > 180.0 {
			self.settings.rotation.y = -R180 + self.settings.rotation.y % R180;
		}

		self.is_dirty = true;
	}

	/// Move the camera by `delta`, relative to the camera's rotation
	pub fn move_camera_relative(&mut self, delta: Vector3) {
		let rot_matrix = Mat4::from_euler_angles(
			-self.settings.rotation.x,
			self.settings.rotation.y,
			self.settings.rotation.z
		);

		let new = rot_matrix * Vec4::new(delta.x, delta.y, delta.z, 1.0);
		self.settings.position.x += new.x;
		self.settings.position.y += new.y;
		self.settings.position.z += new.z;

		self.is_dirty = true;
	}

	pub fn camera_pos(&self) -> Vector3 {
		self.settings.position
	}
}

#[cfg(test)]
mod tests {
	use stockton_types::Matrix4;
use stockton_types::Vector3;
	use draw::camera::WorkingCamera;

	fn contains_nan(mat: &Matrix4) -> bool{
		for x in mat.iter() {
			if *x == std::f32::NAN {
				return true;
			}
		}
		return false;
	}

	#[test]
	fn camera_vp() {
		let mut camera = WorkingCamera::defaults(16.0 / 9.0);

		let old = camera.calc_vp_matrix();
		println!("initial vp matrix: {:?}", old);

		assert!(!contains_nan(&old), "No NaNs for initial matrix");

		// Do a 180
		camera.rotate(Vector3::new(0.0, 180.0, 0.0));

		let new = camera.calc_vp_matrix();
		assert!(!contains_nan(&new), "No NaNs after rotating");

		println!("new vp matrix: {:?}", new);

		assert!(old != new, "VP Matrix changes when camera rotates");
	}
}
