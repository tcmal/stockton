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
use std::ops::{Index, IndexMut, Range};
use std::convert::TryInto;
use core::mem::{ManuallyDrop, size_of};
use hal::memory::{Pod, Properties, Requirements};
use hal::buffer::Usage;
use hal::adapter::MemoryTypeId;
use hal::{VertexCount, InstanceCount, Adapter, Device, PhysicalDevice, mapping};
use back::Backend;
use crate::error::CreationError;
use super::RenderingContext;

pub(crate) struct VertexLump<T: Into<X>, X: Pod> {
	pub (crate) buffer: ManuallyDrop<<Backend as hal::Backend>::Buffer>,
	memory: ManuallyDrop<<Backend as hal::Backend>::Memory>,
	requirements: Requirements,

	unit_size_bytes: u64,
	unit_size_verts: u64,
	batch_size: u64,

	num_batches: usize,


	/// An instance is active if it has been assigned to
	pub active_instances: Range<InstanceCount>,
	pub active_verts: Range<VertexCount>,

	active: bool,

	_t: PhantomData<T>,
	_x: PhantomData<X>
}

const BATCH_SIZE: u64 = 3;

impl<T: Into<X>, X: Pod> VertexLump<T, X> {
	pub fn new(device: &mut <Backend as hal::Backend>::Device, adapter: &Adapter<Backend>) -> Result<VertexLump<T, X>, CreationError> {
		let unit_size_bytes = size_of::<X>()  as u64;
		let unit_size_verts = unit_size_bytes / size_of::<f32>() as u64;

		let mut buffer = unsafe { device
		        .create_buffer(BATCH_SIZE * unit_size_bytes, Usage::VERTEX) }

	        .map_err(|e| CreationError::BufferError (e))?;

		let requirements = unsafe { device.get_buffer_requirements(&buffer) };
		let memory_type_id = adapter.physical_device
	        .memory_properties().memory_types
	        .iter().enumerate()
	        .find(|&(id, memory_type)| {
	        	requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(Properties::CPU_VISIBLE)
	        })
	        .map(|(id, _)| MemoryTypeId(id))
	        .ok_or(CreationError::BufferNoMemory)?;

		let memory = unsafe {device
			.allocate_memory(memory_type_id, requirements.size) }
			.map_err(|_| CreationError::OutOfMemoryError)?;

		unsafe { device
			.bind_buffer_memory(&memory, 0, &mut buffer) }
			.map_err(|_| CreationError::BufferNoMemory)?;

		Ok(VertexLump {
			buffer: ManuallyDrop::new(buffer),
			memory: ManuallyDrop::new(memory),
			requirements,
			active_verts: 0..0,
			active_instances: 0..0,
			num_batches: 1,
			unit_size_bytes,
			unit_size_verts,
			batch_size: BATCH_SIZE, // TODO
			active: true,
			_t: PhantomData,
			_x: PhantomData
		})
	}

	pub fn set_active_instances(&mut self, range: Range<InstanceCount>) {
		let count: u64 = (range.end - range.start).into();
		let size_verts: u32 = (count * self.unit_size_verts).try_into().unwrap();
		self.active_verts = range.start * size_verts..range.end * size_verts;
		self.active_instances = range;
	}

	pub fn add(&mut self, tri: T, ctx: &mut RenderingContext) -> Result<(), ()> { 

		// figure out where to put it
		let idx: usize = (self.active_instances.end).try_into().unwrap();
		let batch_size: usize = self.batch_size.try_into().unwrap();
		let max_size: usize = self.num_batches * batch_size;

		// make sure correct size
		if idx >= max_size {
			self.num_batches += 1;

			debug!("Reallocating Vertex buffer to {} batches ({} instances)", self.num_batches, self.num_batches as u64 * self.batch_size);
			// get new buffer
			let (new_buffer, new_requirements, new_memory) = {
				let mut buffer = ManuallyDrop::new(unsafe { ctx.device
				        .create_buffer(self.batch_size * self.unit_size_bytes * self.num_batches as u64, Usage::VERTEX) }
			        	.map_err(|_| ())?
	        	);
				let requirements = unsafe { ctx.device.get_buffer_requirements(&buffer) };

				let memory_type_id = ctx.adapter.physical_device
			        .memory_properties().memory_types
			        .iter().enumerate()
			        .find(|&(id, memory_type)| {
			        	requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(Properties::CPU_VISIBLE)
			        })
			        .map(|(id, _)| MemoryTypeId(id))
			        .ok_or(())?;

				let memory = ManuallyDrop::new(unsafe { ctx.device
					.allocate_memory(memory_type_id, requirements.size) }
					.map_err(|_| ())?);

				unsafe { ctx.device
					.bind_buffer_memory(&memory, 0, &mut buffer) }
					.map_err(|_| ())?;
				
				(buffer, requirements, memory)
			};

			// copy vertices
			unsafe {
				let copy_range = 0..self.requirements.size;

				trace!("Copying {:?} from old buffer to new buffer", copy_range);

				let reader = ctx.device.acquire_mapping_reader::<u8>(&self.memory, copy_range.clone())
					.map_err(|_| ())?;
				let mut writer = ctx.device.acquire_mapping_writer::<u8>(&new_memory, copy_range.clone())
					.map_err(|_| ())?;

				let copy_range: Range<usize> = 0..self.requirements.size.try_into().unwrap();
				writer[copy_range.clone()].copy_from_slice(&reader[copy_range.clone()]);

				ctx.device.release_mapping_reader(reader);
				ctx.device.release_mapping_writer(writer).map_err(|_| ())?;
			};

			// destroy old buffer
			self.deactivate(ctx);

			// use new one
			self.buffer = new_buffer;
			self.requirements = new_requirements;
			self.memory = new_memory;
			self.active = true;

		}

		{
			// acquire writer
			let mut writer = self.writer(ctx)?;

			// write to it
			writer[idx] = tri.into();
		}

		// activate new triangle
		let new_range = self.active_instances.start..self.active_instances.end + 1;
		self.set_active_instances(new_range);

		Ok(())
	}

	pub(crate) fn writer<'a>(&'a mut self, ctx: &'a mut RenderingContext) -> Result<VertexWriter<'a, X>, ()> {
		let mapping_writer = unsafe { ctx.device
			.acquire_mapping_writer(&self.memory, 0..self.requirements.size)
			.map_err(|_| ())? };
		
		Ok(VertexWriter {
			mapping_writer: ManuallyDrop::new(mapping_writer),
			ctx
		})
	}

	pub(crate) fn deactivate(&mut self, ctx: &mut RenderingContext) {
		unsafe { ctx.device.free_memory(ManuallyDrop::take(&mut self.memory)) };
		unsafe { ctx.device.destroy_buffer(ManuallyDrop::take(&mut self.buffer)) };
		self.active = false;
	}
}

pub struct VertexWriter<'a, X: Pod> {
	mapping_writer: ManuallyDrop<mapping::Writer<'a, Backend, X>>,
	ctx: &'a mut RenderingContext
}

impl<'a, X: Pod> Drop for VertexWriter<'a, X> {
	fn drop(&mut self) {
		unsafe {
			self.ctx.device.release_mapping_writer(ManuallyDrop::take(&mut self.mapping_writer))
		}.unwrap();
	}
}

impl<'a, X: Pod> Index<usize> for VertexWriter<'a, X> {
	type Output = X;

	fn index(&self, index: usize) -> &Self::Output {
		&self.mapping_writer[index]
	}
}

impl<'a, X: Pod> IndexMut<usize> for VertexWriter<'a, X> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		&mut self.mapping_writer[index]
	}
} 


impl<T: Into<X>, X: Pod> Drop for VertexLump<T, X> {
	fn drop(&mut self) {
		if self.active {
			panic!("VertexLump dropped without being deactivated");
		}
	}
}