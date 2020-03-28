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

use std::ops::{Index, IndexMut};
use std::convert::TryInto;
use core::mem::{ManuallyDrop, size_of};

use hal::prelude::*;
use hal::{
	MemoryTypeId,
	buffer::Usage,
	memory::{Properties, Segment},
	queue::Submission
};

use crate::error::CreationError;
use crate::types::*;

// TODO: Proper sizing of buffers
const BUF_SIZE: u64 = 4 * 15 * 2;

fn create_buffer(device: &mut Device,
	adapter: &Adapter,
	usage: Usage,
	properties: Properties) -> Result<(Buffer, Memory), CreationError> {
	let mut buffer = unsafe { device
	        .create_buffer(BUF_SIZE, usage) }
	    .map_err(|e| CreationError::BufferError (e))?;

	let requirements = unsafe { device.get_buffer_requirements(&buffer) };
	let memory_type_id = adapter.physical_device
	    .memory_properties().memory_types
	    .iter().enumerate()
	    .find(|&(id, memory_type)| {
	    	requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(properties)
	    })
	    .map(|(id, _)| MemoryTypeId(id))
	    .ok_or(CreationError::BufferNoMemory)?;

	let memory = unsafe {device
		.allocate_memory(memory_type_id, requirements.size) }
		.map_err(|_| CreationError::OutOfMemoryError)?;

	unsafe { device
		.bind_buffer_memory(&memory, 0, &mut buffer) }
		.map_err(|_| CreationError::BufferNoMemory)?;

	Ok((buffer, memory
))
}

pub trait ModifiableBuffer: IndexMut<usize> {
	fn commit<'a>(&'a mut self, device: &Device,
		command_queue: &mut CommandQueue, 
		command_pool: &mut CommandPool) -> &'a Buffer;
}

pub struct StagedBuffer<'a, T: Sized> {
	staged_buffer: ManuallyDrop<Buffer>,
	staged_memory: ManuallyDrop<Memory>,
	buffer: ManuallyDrop<Buffer>,
	memory: ManuallyDrop<Memory>,
	staged_mapped_memory: &'a mut [T],
	staged_is_dirty: bool,
}


impl<'a, T: Sized> StagedBuffer<'a, T> {
	pub fn new(device: &mut Device, adapter: &Adapter, usage: Usage) -> Result<Self, CreationError> {

		let (staged_buffer, staged_memory) = create_buffer(device, adapter, Usage::TRANSFER_SRC, Properties::CPU_VISIBLE)?;
		let (buffer, memory) = create_buffer(device, adapter, Usage::TRANSFER_DST | usage, Properties::DEVICE_LOCAL)?;

		// Map it somewhere and get a slice to that memory
		let staged_mapped_memory = unsafe {
			let ptr = device.map_memory(&staged_memory, Segment::ALL).unwrap();
			
			let slice_size: usize = (BUF_SIZE / size_of::<T>() as u64).try_into().unwrap(); // size in f32s

			std::slice::from_raw_parts_mut(ptr as *mut T, slice_size)
		};

		Ok(StagedBuffer {
			staged_buffer: ManuallyDrop::new(staged_buffer),
			staged_memory: ManuallyDrop::new(staged_memory),
			buffer: ManuallyDrop::new(buffer),
			memory: ManuallyDrop::new(memory),
			staged_mapped_memory,
			staged_is_dirty: false
		})
	}

	pub(crate) fn deactivate(mut self, device: &mut Device) {
		unsafe { 
			device.unmap_memory(&self.staged_memory);

			device.free_memory(ManuallyDrop::take(&mut self.staged_memory));
			device.destroy_buffer(ManuallyDrop::take(&mut self.staged_buffer));

			device.free_memory(ManuallyDrop::take(&mut self.memory));
			device.destroy_buffer(ManuallyDrop::take(&mut self.buffer));
		};
	}
}

impl <'a, T: Sized> ModifiableBuffer for StagedBuffer<'a, T> {
	fn commit<'b>(&'b mut self, device: &Device,
		command_queue: &mut CommandQueue, 
		command_pool: &mut CommandPool) -> &'b Buffer {
		if self.staged_is_dirty {
			// Copy from staged to buffer

			let buf = unsafe {
				use hal::command::{CommandBufferFlags, BufferCopy};
				// Get a command buffer
				let mut buf = command_pool.allocate_one(hal::command::Level::Primary);

				// Put in our copy command
				buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);
				buf.copy_buffer(&self.staged_buffer, &self.buffer, &[
					BufferCopy {
						src: 0,
						dst: 0,
						size: BUF_SIZE // TODO
					}
				]);
				buf.finish();

				buf
			};

			// Submit it and wait for completion
			// TODO: We could use more semaphores or something?
			// TODO: Better error handling
			unsafe {
				let copy_finished = device.create_fence(false).unwrap();
				command_queue.submit::<_, _, Semaphore, _, _>(Submission {
					command_buffers: &[&buf],
					wait_semaphores: std::iter::empty::<_>(),
					signal_semaphores: std::iter::empty::<_>()
				}, Some(&copy_finished));

				device
			        .wait_for_fence(&copy_finished, core::u64::MAX).unwrap();
				device.destroy_fence(copy_finished);
			}

			self.staged_is_dirty = false;
		}

		&self.buffer
	}
}

impl<'a, T: Sized> Index<usize> for StagedBuffer<'a, T> {
	type Output = T;

	fn index(&self, index: usize) -> &Self::Output {
		&self.staged_mapped_memory[index]
	}
}

impl<'a, T: Sized> IndexMut<usize> for StagedBuffer<'a, T> {
	fn index_mut(&mut self, index: usize) -> &mut Self::Output {
		self.staged_is_dirty = true;
		&mut self.staged_mapped_memory[index]
	}
}

// trait VertexLump {
// 	pub fn new(device: &mut Device, adapter: &Adapter<back::Backend>) -> Result<Self, CreationError> {
// }

// pub(crate) struct VertexLump<T: Into<X>, X: Pod> {
// 	pub (crate) buffer: ManuallyDrop<Buffer>,
// 	memory: ManuallyDrop<Memory>,
// 	requirements: Requirements,

// 	unit_size_bytes: u64,
// 	unit_size_verts: u64,
// 	batch_size: u64,

// 	num_batches: usize,


// 	/// An instance is active if it has been assigned to
// 	pub active_instances: Range<InstanceCount>,
// 	pub active_verts: Range<VertexCount>,

// 	active: bool,

// 	_t: PhantomData<T>,
// 	_x: PhantomData<X>
// }

// const BATCH_SIZE: u64 = 3;

// impl<T: Into<X>, X: Pod> VertexLump<T, X> {
// 	pub fn new(device: &mut Device, adapter: &Adapter<back::Backend>) -> Result<VertexLump<T, X>, CreationError> {
// 		let unit_size_bytes = size_of::<X>()  as u64;
// 		let unit_size_verts = unit_size_bytes / size_of::<f32>() as u64;

// 		let mut buffer = unsafe { device
// 		        .create_buffer(BATCH_SIZE * unit_size_bytes, Usage::VERTEX) }

// 	        .map_err(|e| CreationError::BufferError (e))?;

// 		let requirements = unsafe { device.get_buffer_requirements(&buffer) };
// 		let memory_type_id = adapter.physical_device
// 	        .memory_properties().memory_types
// 	        .iter().enumerate()
// 	        .find(|&(id, memory_type)| {
// 	        	requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(Properties::CPU_VISIBLE)
// 	        })
// 	        .map(|(id, _)| MemoryTypeId(id))
// 	        .ok_or(CreationError::BufferNoMemory)?;

// 		let memory = unsafe {device
// 			.allocate_memory(memory_type_id, requirements.size) }
// 			.map_err(|_| CreationError::OutOfMemoryError)?;

// 		unsafe { device
// 			.bind_buffer_memory(&memory, 0, &mut buffer) }
// 			.map_err(|_| CreationError::BufferNoMemory)?;

// 		Ok(VertexLump {
// 			buffer: ManuallyDrop::new(buffer),
// 			memory: ManuallyDrop::new(memory),
// 			requirements,
// 			active_verts: 0..0,
// 			active_instances: 0..0,
// 			num_batches: 1,
// 			unit_size_bytes,
// 			unit_size_verts,
// 			batch_size: BATCH_SIZE, // TODO
// 			active: true,
// 			_t: PhantomData,
// 			_x: PhantomData
// 		})
// 	}

// 	pub fn set_active_instances(&mut self, range: Range<InstanceCount>) {
// 		let count: u64 = (range.end - range.start).into();
// 		let size_verts: u32 = (count * self.unit_size_verts).try_into().unwrap();
// 		self.active_verts = range.start * size_verts..range.end * size_verts;
// 		self.active_instances = range;
// 	}

// 	pub fn add(&mut self, tri: T, ctx: &mut RenderingContext) -> Result<(), ()> { 

// 		// figure out where to put it
// 		let idx: usize = (self.active_instances.end).try_into().unwrap();
// 		let batch_size: usize = self.batch_size.try_into().unwrap();
// 		let max_size: usize = self.num_batches * batch_size;

// 		// make sure correct size
// 		if idx >= max_size {
// 			self.num_batches += 1;

// 			debug!("Reallocating Vertex buffer to {} batches ({} instances)", self.num_batches, self.num_batches as u64 * self.batch_size);
// 			// get new buffer
// 			let (new_buffer, new_requirements, new_memory) = {
// 				let mut buffer = ManuallyDrop::new(unsafe { ctx.device
// 				        .create_buffer(self.batch_size * self.unit_size_bytes * self.num_batches as u64, Usage::VERTEX) }
// 			        	.map_err(|_| ())?
// 	        	);
// 				let requirements = unsafe { ctx.device.get_buffer_requirements(&buffer) };

// 				let memory_type_id = ctx.adapter.physical_device
// 			        .memory_properties().memory_types
// 			        .iter().enumerate()
// 			        .find(|&(id, memory_type)| {
// 			        	requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(Properties::CPU_VISIBLE)
// 			        })
// 			        .map(|(id, _)| MemoryTypeId(id))
// 			        .ok_or(())?;

// 				let memory = ManuallyDrop::new(unsafe { ctx.device
// 					.allocate_memory(memory_type_id, requirements.size) }
// 					.map_err(|_| ())?);

// 				unsafe { ctx.device
// 					.bind_buffer_memory(&memory, 0, &mut buffer) }
// 					.map_err(|_| ())?;
				
// 				(buffer, requirements, memory)
// 			};

// 			// copy vertices
// 			unsafe {
// 				let copy_range = 0..self.requirements.size;

// 				trace!("Copying {:?} from old buffer to new buffer", copy_range);

// 				let reader = ctx.device.acquire_mapping_reader::<u8>(&*(self.memory), copy_range.clone())
// 					.map_err(|_| ())?;
// 				let mut writer = ctx.device.acquire_mapping_writer::<u8>(&new_memory, copy_range.clone())
// 					.map_err(|_| ())?;

// 				let copy_range: Range<usize> = 0..self.requirements.size.try_into().unwrap();
// 				writer[copy_range.clone()].copy_from_slice(&reader[copy_range.clone()]);

// 				ctx.device.release_mapping_reader(reader);
// 				ctx.device.release_mapping_writer(writer).map_err(|_| ())?;
// 			};

// 			// destroy old buffer
// 			self.deactivate(ctx);

// 			// use new one
// 			self.buffer = new_buffer;
// 			self.requirements = new_requirements;
// 			self.memory = new_memory;
// 			self.active = true;

// 		}

// 		{
// 			// acquire writer
// 			let mut writer = self.writer(ctx)?;

// 			// write to it
// 			writer[idx] = tri.into();
// 		}

// 		// activate new triangle
// 		let new_range = self.active_instances.start..self.active_instances.end + 1;
// 		self.set_active_instances(new_range);

// 		Ok(())
// 	}

// 	pub(crate) fn writer<'a>(&'a mut self, ctx: &'a mut RenderingContext) -> Result<VertexWriter<'a, X>, ()> {
// 		let mapping_writer = unsafe { ctx.device
// 			.acquire_mapping_writer(&*(self.memory), 0..self.requirements.size)
// 			.map_err(|_| ())? };
		
// 		Ok(VertexWriter {
// 			mapping_writer: ManuallyDrop::new(mapping_writer),
// 			ctx
// 		})
// 	}

// }

