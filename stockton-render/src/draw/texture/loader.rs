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
//! Deals with loading textures into GPU memory

use std::path::Path;
use draw::texture::resolver::BasicFSResolver;
use draw::texture::chunk::CHUNK_SIZE;
use core::mem::{ManuallyDrop};
use super::chunk::TextureChunk;

use log::debug;

use hal::{
	prelude::*,
};

use stockton_levels::prelude::*;

use crate::error;
use crate::types::*;

/// Stores all loaded textures in GPU memory.
/// When rendering, the descriptor sets are bound to the buffer
/// The descriptor set layout should have the same count of textures as this does.
/// All descriptors will be properly initialised images.
pub struct TextureStore {
	descriptor_pool: ManuallyDrop<DescriptorPool>,
	pub(crate) descriptor_set_layout: ManuallyDrop<DescriptorSetLayout>,
	chunks: Box<[TextureChunk]>
}

impl TextureStore {
	/// Create a new texture store for the given file, loading all textures from it.
	pub fn new<T: HasTextures>(device: &mut Device,
		adapter: &mut Adapter,
		command_queue: &mut CommandQueue,
		command_pool: &mut CommandPool, file: &T) -> Result<TextureStore, error::CreationError> {
		// Figure out how many textures in this file / how many chunks needed
		let size = file.textures_iter().count();
		let num_chunks = {
			let mut x = size / CHUNK_SIZE;
			if size % CHUNK_SIZE != 0 {
				x += 1;
			}
			x
		};
		let rounded_size = num_chunks * CHUNK_SIZE;

		// Descriptor pool, where we get our sets from
		let mut descriptor_pool = unsafe {
			use hal::pso::{DescriptorRangeDesc, DescriptorType, DescriptorPoolCreateFlags, ImageDescriptorType};

			device.create_descriptor_pool(
				num_chunks,
				&[
					DescriptorRangeDesc {
						ty: DescriptorType::Image {
							ty: ImageDescriptorType::Sampled {
								with_sampler: false
							}
						},
						count: rounded_size
					},
					DescriptorRangeDesc {
						ty: DescriptorType::Sampler,
						count: rounded_size
					}
				],
				DescriptorPoolCreateFlags::empty()
			).map_err(|e| {
				println!("{:?}", e);
				error::CreationError::OutOfMemoryError
			})?
		};

		// Layout of our descriptor sets
		let mut descriptor_set_layout = unsafe {
			use hal::pso::{DescriptorSetLayoutBinding, DescriptorType, ShaderStageFlags, ImageDescriptorType};

			device.create_descriptor_set_layout(
				&[
					DescriptorSetLayoutBinding {
						binding: 0,
						ty: DescriptorType::Image {
							ty: ImageDescriptorType::Sampled {
								with_sampler: false
							}
						},
						count: CHUNK_SIZE,
						stage_flags: ShaderStageFlags::FRAGMENT,
						immutable_samplers: false
					},
					DescriptorSetLayoutBinding {
						binding: 1,
						ty: DescriptorType::Sampler,
						count: CHUNK_SIZE,
						stage_flags: ShaderStageFlags::FRAGMENT,
						immutable_samplers: false
					}
				],
				&[],
			)
		}.map_err(|_| error::CreationError::OutOfMemoryError)?;

		// TODO: Proper way to set up resolver
		let mut resolver = BasicFSResolver::new(Path::new("."));

		// Create texture chunks
		debug!("Starting to load textures...");
		let mut chunks = Vec::with_capacity(num_chunks);
		for i in 0..num_chunks {
			let range = {
				let mut r = (i * CHUNK_SIZE) as u32..((i + 1) * CHUNK_SIZE) as u32;
				if r.end > size as u32 {
					r.end = size as u32;
				}
				r
			};
			debug!("Chunk {} / {} covering {:?}", i + 1, num_chunks, range);

			chunks.push(
				TextureChunk::new(
					device, adapter, command_queue,
					command_pool, &mut descriptor_pool,
					&mut descriptor_set_layout, file,
					range, &mut resolver
				)?
			);
		}

		debug!("All textures loaded.");

		Ok(TextureStore {
			descriptor_pool: ManuallyDrop::new(descriptor_pool),
			descriptor_set_layout: ManuallyDrop::new(descriptor_set_layout),
			chunks: chunks.into_boxed_slice()
		})
	}

	/// Call this before dropping
	pub fn deactivate(mut self, device: &mut Device) -> () {
		unsafe {
			use core::ptr::read;

			for chunk in self.chunks.into_vec().drain(..) {
				chunk.deactivate(device)
			}

			self.descriptor_pool.reset();
			device
				.destroy_descriptor_set_layout(ManuallyDrop::into_inner(read(&self.descriptor_set_layout)));
			device.destroy_descriptor_pool(ManuallyDrop::into_inner(read(&self.descriptor_pool)));
		}
	}

	/// Get number of chunks being used
	pub fn get_n_chunks(&self) -> usize {
		self.chunks.len()
	}

	/// Get the descriptor set for a given chunk
	pub fn get_chunk_descriptor_set<'a>(&'a self, idx: usize) -> &'a DescriptorSet {
		&self.chunks[idx].descriptor_set
	}
}
