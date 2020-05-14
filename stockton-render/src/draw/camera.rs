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

use std::iter::once;
use std::f32::consts::PI;
use hal::prelude::*;
use hal::buffer::Usage;
use na::{look_at_lh, perspective_lh_zo, radians};

use core::mem::ManuallyDrop;

use crate::error;
use crate::types::*;
use super::buffer::{StagedBuffer, ModifiableBuffer};
use stockton_types::{Vector3, Matrix4};
use na::{Mat4, Vec4};

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
pub struct WorkingCamera<'a> {
	/// Settings for the camera
	settings: CameraSettings,

	/// Aspect ratio as a fraction
	aspect_ratio: f32,

	/// Layout of the descriptor set to pass to the shader
	pub descriptor_set_layout: ManuallyDrop<DescriptorSetLayout>,

	/// Buffer of memory used for passing data to shaders
	// TODO: Does this need to be staged?
	buffer: ManuallyDrop<StagedBuffer<'a, Matrix4>>,

	// TODO: Share descriptor pool with textures?
	descriptor_pool: ManuallyDrop<DescriptorPool>,
	descriptor_set: DescriptorSet,

	/// If true, buffer needs updated
	is_dirty: bool
}

impl<'a> WorkingCamera<'a> {
	pub fn defaults(aspect_ratio: f32, device: &mut Device, adapter: &Adapter,
		command_queue: &mut CommandQueue, 
		command_pool: &mut CommandPool) -> Result<WorkingCamera<'a>, error::CreationError> {
		WorkingCamera::with_settings(CameraSettings {
			position: Vector3::new(0.0, 0.0, 0.0),
			rotation: Vector3::new(0.0, R90, 0.0),
			up: Vector3::new(0.0, 1.0, 0.0),
			fov: f32::to_radians(90.0),
			near: 0.1,
			far: 1024.0,
		}, aspect_ratio, device, adapter, command_queue, command_pool)
	}

	/// Return a camera with default settings
	// TODO
	pub fn with_settings(settings: CameraSettings, aspect_ratio: f32, device: &mut Device, adapter: &Adapter,
		command_queue: &mut CommandQueue, 
		command_pool: &mut CommandPool) -> Result<WorkingCamera<'a>, error::CreationError> {

		let descriptor_type = {
			use hal::pso::{DescriptorType, BufferDescriptorType, BufferDescriptorFormat};

			DescriptorType::Buffer {
				ty: BufferDescriptorType::Uniform,
				format: BufferDescriptorFormat::Structured {
					dynamic_offset: false
				}
			}
		};

		// Create set layout
		let descriptor_set_layout = unsafe {
			use hal::pso::{DescriptorSetLayoutBinding, ShaderStageFlags};

			device.create_descriptor_set_layout(
				&[
					DescriptorSetLayoutBinding {
						binding: 0,
						ty: descriptor_type,
						count: 1,
						stage_flags: ShaderStageFlags::VERTEX,
						immutable_samplers: false
					}
				],
				&[],
			)
		}.map_err(|_| error::CreationError::OutOfMemoryError)?;

		// Create pool and allocate set
		let (descriptor_pool, descriptor_set) = unsafe {
			use hal::pso::{DescriptorRangeDesc, DescriptorPoolCreateFlags};

			let mut pool = device.create_descriptor_pool(
				1,
				&[
					DescriptorRangeDesc {
						ty: descriptor_type,
						count: 1
					}
				],
				DescriptorPoolCreateFlags::empty()
			).map_err(|_| error::CreationError::OutOfMemoryError)?;

			let set = pool.allocate_set(&descriptor_set_layout).map_err(|_| error::CreationError::OutOfMemoryError)?;

			(pool, set)
		};
		
		// Create buffer for descriptor
		let mut buffer = StagedBuffer::new(device, adapter, Usage::UNIFORM, 1)?;

		// Bind our buffer to our descriptor set
		unsafe {
			use hal::pso::{Descriptor, DescriptorSetWrite};
			use hal::buffer::SubRange;

			device.write_descriptor_sets(once(
				DescriptorSetWrite {
					set: &descriptor_set,
					binding: 0,
					array_offset: 0,
					descriptors: once(
						Descriptor::Buffer(buffer.commit(device, command_queue, command_pool), SubRange::WHOLE)
					)
				}
			));
		}

		Ok(WorkingCamera {
			aspect_ratio,
			settings,

			descriptor_set_layout: ManuallyDrop::new(descriptor_set_layout),
			buffer: ManuallyDrop::new(buffer),

			descriptor_pool: ManuallyDrop::new(descriptor_pool),
			descriptor_set: descriptor_set,

			is_dirty: true
		})
	}

	/// Returns a matrix that transforms from world space to screen space
	pub fn vp_matrix(&self) -> Matrix4 {
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
		if self.settings.rotation.x > R90 {
			self.settings.rotation.x = R90;
		} else if self.settings.rotation.x < -R90 {
			self.settings.rotation.x = -R90;
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

	/// Ensures the VP matrix on the GPU is up-to-date
	pub fn commit<'b>(&'b mut self, device: &Device,
		command_queue: &mut CommandQueue, 
		command_pool: &mut CommandPool) -> &'b DescriptorSet {
		// Update buffer if needed
		if self.is_dirty {
			self.buffer[0] = self.vp_matrix();
			self.buffer.commit(device, command_queue, command_pool);

			self.is_dirty = false;
		}

		// Return the descriptor set for matrices
		&self.descriptor_set
	}

	/// This should be called before dropping
	pub fn deactivate(mut self, device: &mut Device) -> () {
		unsafe {
			use core::ptr::read;

			ManuallyDrop::into_inner(read(&self.buffer)).deactivate(device);

			self.descriptor_pool.reset();
			device.destroy_descriptor_pool(ManuallyDrop::into_inner(read(&self.descriptor_pool)));
			device.destroy_descriptor_set_layout(ManuallyDrop::into_inner(read(&self.descriptor_set_layout)));
		}
	}

	pub fn camera_pos(&self) -> Vector3 {
		self.settings.position
	}
}