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

//! A chunk of textures is an array of textures, the size of which is known at compile time.
//! This reduces the number of times we need to re-bind our descriptor sets

use image::{Rgba, RgbaImage};
use hal::prelude::*;

use core::{
	mem::{replace}
};
use std::ops::{Range, Deref};

use crate::{
	types::*,
	error
};

use log::debug;
use super::resolver::TextureResolver;
use super::image::LoadedImage;
use stockton_levels::prelude::*;

/// The size of a chunk. Needs to match up with the fragment shader
pub const CHUNK_SIZE: usize = 8;

/// An array of textures
pub struct TextureChunk {
	pub(crate) descriptor_set: DescriptorSet,
	loaded_images: Vec<LoadedImage>,
}

impl TextureChunk {
	pub fn new<T: HasTextures, R: TextureResolver>(device: &mut Device,
		adapter: &mut Adapter,
		command_queue: &mut CommandQueue,
		command_pool: &mut CommandPool, 
		pool: &mut DescriptorPool,
		layout: &DescriptorSetLayout,
		file: &T, range: Range<u32>,
		resolver: &mut R) -> Result<TextureChunk, error::CreationError> {

		let descriptor_set = unsafe {
			pool.allocate_set(&layout).map_err(|e| {
				println!("{:?}", e);
				error::CreationError::OutOfMemoryError
			})?
		};

		let mut store = TextureChunk {
			descriptor_set: descriptor_set,
			loaded_images: Vec::with_capacity(CHUNK_SIZE),
		};

		let mut local_idx = 0;

		debug!("Created descriptor set");
		for tex_idx in range {
			debug!("Loading tex {}", local_idx + 1);
			let tex = file.get_texture(tex_idx);
			let img = resolver.resolve(tex);
			store.put_texture(img, local_idx,
				device, adapter,
				command_queue, command_pool).unwrap();

			local_idx += 1;
		}

		// Pad out the end if needed
		while local_idx < CHUNK_SIZE {
			debug!("Putting a placeholder in slot {}", local_idx);
			store.put_texture(RgbaImage::from_pixel(1, 1, Rgba ([0, 0, 0, 1])), local_idx,
				device, adapter,
				command_queue, command_pool).unwrap();

			local_idx += 1;
		}

		Ok(store)
	}


	pub fn put_texture(&mut self, image: RgbaImage,
		idx: usize,
		device: &mut Device,
		adapter: &mut Adapter,
		command_queue: &mut CommandQueue,
		command_pool: &mut CommandPool) -> Result<(), &'static str>{

		// Load the image
		let texture = LoadedImage::load(
			image,
			device,
			adapter,
			command_queue,
			command_pool,
		)?;

		// Write it to the descriptor set
		unsafe {
			use hal::pso::{DescriptorSetWrite, Descriptor};
			use hal::image::Layout;

			device.write_descriptor_sets(vec![
				DescriptorSetWrite {
					set: &self.descriptor_set,
					binding: 0,
					array_offset: idx,
					descriptors: Some(Descriptor::Image(
						texture.image_view.deref(),
						Layout::ShaderReadOnlyOptimal
					)),
				},
				DescriptorSetWrite {
					set: &self.descriptor_set,
					binding: 1,
					array_offset: idx,
					descriptors: Some(Descriptor::Sampler(texture.sampler.deref())),
				},
			]);
		};

		// Store it so we can safely deactivate it when we need to
		// Deactivate the old image if we need to
		if idx < self.loaded_images.len() {
			replace(&mut self.loaded_images[idx], texture).deactivate(device);
		} else {
			self.loaded_images.push(texture);
		}

		Ok(())
	}

	pub fn deactivate(mut self, device: &mut Device) -> () {
		for img in self.loaded_images.drain(..) {
			img.deactivate(device);
		}
	}
}