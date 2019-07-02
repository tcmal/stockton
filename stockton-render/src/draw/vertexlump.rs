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

use std::marker::PhantomData;
use core::mem::{size_of};
use hal::memory::{Pod, Properties, Requirements};
use hal::buffer::Usage;
use hal::adapter::MemoryTypeId;
use hal::{Device, PhysicalDevice, mapping};
use back::Backend;
use crate::error::CreationError;
use super::RenderingContext;

pub struct VertexLump<T: Into<X>, X: Pod> {
	buffer: <Backend as hal::Backend>::Buffer,
	memory: <Backend as hal::Backend>::Memory,
	requirements: Requirements,

	unit_size_bytes: u64,
	batch_size: u64,

	active: bool,

	_t: PhantomData<T>,
	_x: PhantomData<X>,
}

const BATCH_SIZE: u64 = 3;

impl<T: Into<X>, X: Pod> VertexLump<T, X> {
	pub(crate) fn new(ctx: &mut RenderingContext) -> Result<VertexLump<T, X>, CreationError> {
		let unit_size_bytes = size_of::<X>() as u64;

		let mut buffer = unsafe { ctx.device
		        .create_buffer(BATCH_SIZE * unit_size_bytes, Usage::VERTEX) }

	        .map_err(|e| CreationError::BufferError (e))?;

		let requirements = unsafe { ctx.device.get_buffer_requirements(&buffer) };
		let memory_type_id = ctx.adapter.physical_device
	        .memory_properties().memory_types
	        .iter().enumerate()
	        .find(|&(id, memory_type)| {
	        	requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(Properties::CPU_VISIBLE)
	        })
	        .map(|(id, _)| MemoryTypeId(id))
	        .ok_or(CreationError::BufferNoMemory)?;

		let memory = unsafe {ctx.device
			.allocate_memory(memory_type_id, requirements.size) }
			.map_err(|_| CreationError::OutOfMemoryError)?;

		unsafe { ctx.device
			.bind_buffer_memory(&memory, 0, &mut buffer) }
			.map_err(|_| CreationError::BufferNoMemory)?;

		Ok(VertexLump {
			buffer, memory, requirements,

			unit_size_bytes,
			batch_size: BATCH_SIZE, // TODO
			active: true,
			_t: PhantomData,
			_x: PhantomData
		})
	}

	pub(crate) fn writer<'a>(&'a self, ctx: &'a mut RenderingContext) -> Result<VertexWriter<'a, X>, ()> {
		let mapping_writer = unsafe { ctx.device
			.acquire_mapping_writer(&self.memory, 0..self.requirements.size)
			.map_err(|_| ())? };

		Ok(VertexWriter {
			mapping_writer,
			ctx
		})
	}
}

// TODO
pub struct VertexWriter<'a, X: Pod> {
	mapping_writer: mapping::Writer<'a, Backend, X>,
	ctx: &'a mut RenderingContext
}

impl<'a, X: Pod> IndexMut<usize> for 

impl<'a, X: Pod> Drop for VertexWriter<'a, X> {
	fn drop(&mut self) {
		// TODO
	}
}


impl<T: Into<X>, X: Pod> Drop for VertexLump<T, X> {
	fn drop(&mut self) {
		if !self.active {
			panic!("VertexLump dropped without being deactivated");
		}
	}
}