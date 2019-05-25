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

//! Helper struct. Keeps the data for each frame 'in flight' seperate instead of linked lists.
use hal::{Device as DeviceTrait};
use hal::command::CommandBuffer;
use hal::Graphics;
use back::{Backend};
use hal::pool::{CommandPool};


/// Helper struct for a frame that can be in flight
pub struct FrameCell {
	/// How we ask the GPU to do work for us.
	pub command_buffer: CommandBuffer<Backend, Graphics>,

	/// Signalled once an image is acquired to draw on.
	pub image_available: <Backend as hal::Backend>::Semaphore,

	/// Signalled once the frame is done being drawn.
	pub render_finished: <Backend as hal::Backend>::Semaphore,

	/// Signalled once the frame is presented.
	pub frame_presented: <Backend as hal::Backend>::Fence
}

impl FrameCell {
	/// Safely deinitialises all the objects in this struct.
	/// Use this instead of drop.
	pub unsafe fn destroy(self, device: &<Backend as hal::Backend>::Device, command_pool: &mut CommandPool<Backend, Graphics>) {
		// fences & semaphores
		device.destroy_semaphore(self.image_available);
		device.destroy_semaphore(self.render_finished);
		device.destroy_fence(self.frame_presented);

		// command buffer
		command_pool.free(vec![self.command_buffer]);
	}
}