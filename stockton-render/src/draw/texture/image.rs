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

use core::{
	ptr::copy_nonoverlapping,
	mem::{size_of, ManuallyDrop}
};
use std::{
	iter::once,
	convert::TryInto
};
use image::RgbaImage;
use hal::{
	MemoryTypeId,
	buffer::Usage as BufUsage,
	format::{Format, Swizzle, Aspects},
	image::{ViewKind, SubresourceRange},
	queue::Submission,
	memory::{Properties as MemProperties, Dependencies as MemDependencies, Segment},
	prelude::*,
};

use crate::types::*;
use draw::buffer::create_buffer;

/// The size of each pixel in an image
const PIXEL_SIZE: usize = size_of::<image::Rgba<u8>>();

/// Holds an image that's loaded into GPU memory and can be sampled from
pub struct LoadedImage {
	/// The GPU Image handle
	image: ManuallyDrop<Image>,

	/// The full view of the image
	pub image_view: ManuallyDrop<ImageView>,

	/// A sampler for the image
	pub sampler: ManuallyDrop<Sampler>,

	/// The memory backing the image
	memory: ManuallyDrop<Memory>
}

impl LoadedImage {
	/// Load the given image into a new buffer
	pub fn load(img: RgbaImage, device: &mut Device, adapter: &Adapter,
		command_queue: &mut CommandQueue,
		command_pool: &mut CommandPool) -> Result<LoadedImage, &'static str> {
		// Round up the size to align properly
		let initial_row_size = PIXEL_SIZE * (img.width() as usize);
		let limits = adapter.physical_device.limits();
		let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;
		
		let row_size = ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
		debug_assert!(row_size as usize >= initial_row_size);

		let total_size = (row_size * img.height() as usize) as u64;

		// Make a staging buffer
		let (staging_buffer, staging_memory) = create_buffer(device, adapter, BufUsage::TRANSFER_SRC, MemProperties::CPU_VISIBLE, total_size)
			.map_err(|_| "Couldn't create staging buffer")?;

		// Copy everything into it
		unsafe {
			let mapped_memory: *mut u8 = device.map_memory(&staging_memory, Segment::ALL).map_err(|_| "Couldn't map buffer memory")?;

			for y in 0..img.height() as usize {
				let row = &(*img)[y * initial_row_size..(y + 1) * initial_row_size];
				let dest_base: isize = (y * row_size).try_into().unwrap();

				copy_nonoverlapping(row.as_ptr(), mapped_memory.offset(dest_base), row.len());
			}
			device.flush_mapped_memory_ranges(once((&staging_memory, Segment::ALL))).map_err(|_| "Couldn't write buffer memory")?;
			device.unmap_memory(&staging_memory);
		}

		// Make the image
		let mut image_ref = unsafe {
			use hal::image::{Kind, Tiling, Usage, ViewCapabilities};

			device.create_image(
				Kind::D2(img.width(), img.height(), 1, 1),
				1,
				Format::Rgba8Srgb,
				Tiling::Optimal,
				Usage::TRANSFER_DST | Usage::SAMPLED,
				ViewCapabilities::empty()
			)
		}.map_err(|_| "Couldn't create image")?;

		// Allocate memory
		let memory = unsafe {
			let requirements = device.get_image_requirements(&image_ref);

			let memory_type_id = adapter.physical_device
				.memory_properties().memory_types
				.iter().enumerate()
				.find(|&(id, memory_type)| {
					requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(MemProperties::DEVICE_LOCAL)
				})
				.map(|(id, _)| MemoryTypeId(id))
				.ok_or("Couldn't find a memory type for image memory")?;

			let memory = device
				.allocate_memory(memory_type_id, requirements.size)
				.map_err(|_| "Couldn't allocate image memory")?;

			device.bind_image_memory(&memory, 0, &mut image_ref)
				.map_err(|_| "Couldn't bind memory to image")?;

			Ok(memory)
		}?;

		// Copy from staging to image memory
		let buf = unsafe {
			use hal::command::{CommandBufferFlags, BufferImageCopy};
			use hal::pso::PipelineStage;
			use hal::memory::Barrier;
			use hal::image::{Access, Layout, SubresourceLayers, Offset, Extent};

			// Get a command buffer
			let mut buf = command_pool.allocate_one(hal::command::Level::Primary);
			buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

			// Setup the layout of our image for copying
			let image_barrier = Barrier::Image {
				states: (Access::empty(), Layout::Undefined)
					..(
						Access::TRANSFER_WRITE,
						Layout::TransferDstOptimal,
					),
				target: &image_ref,
				families: None,
				range: SubresourceRange {
					aspects: Aspects::COLOR,
					levels: 0..1,
					layers: 0..1,
				},
			};
			buf.pipeline_barrier(
				PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
				MemDependencies::empty(),
				&[image_barrier],
			);

			// Copy from buffer to image
			buf.copy_buffer_to_image(&staging_buffer, &image_ref,
				Layout::TransferDstOptimal, &[
				BufferImageCopy {
					buffer_offset: 0,
					buffer_width: (row_size / PIXEL_SIZE) as u32,
					buffer_height: img.height(),
					image_layers: SubresourceLayers {
						aspects: Aspects::COLOR,
						level: 0,
						layers: 0..1
					},
					image_offset: Offset {
						x: 0, y: 0, z: 0
					},
					image_extent: Extent {
						width: img.width(),
						height: img.height(),
						depth: 1
					}
				}
			]);

			// Setup the layout of our image for shaders
			let image_barrier = Barrier::Image {
				states: (
					Access::TRANSFER_WRITE,
					Layout::TransferDstOptimal,
				)..(
					Access::SHADER_READ,
					Layout::ShaderReadOnlyOptimal,
				),
				target: &image_ref,
				families: None,
				range: SubresourceRange {
					aspects: Aspects::COLOR,
					levels: 0..1,
					layers: 0..1,
				},
			};

			buf.pipeline_barrier(
				PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
				MemDependencies::empty(),
				&[image_barrier],
			);

			buf.finish();

			buf
		};

		// Submit our commands and wait for them to finish
		unsafe {
			let setup_finished = device.create_fence(false).unwrap();
			command_queue.submit::<_, _, Semaphore, _, _>(Submission {
				command_buffers: &[&buf],
				wait_semaphores: std::iter::empty::<_>(),
				signal_semaphores: std::iter::empty::<_>()
			}, Some(&setup_finished));

			device
		        .wait_for_fence(&setup_finished, core::u64::MAX).unwrap();
			device.destroy_fence(setup_finished);
		};

		// Clean up temp resources
		unsafe {
			command_pool.free(once(buf));

			device.free_memory(staging_memory);
			device.destroy_buffer(staging_buffer);
		}

		// Create ImageView and sampler
		let image_view = unsafe { device.create_image_view(
			&image_ref,
			ViewKind::D2,
			Format::Rgba8Srgb,
			Swizzle::NO,
			SubresourceRange {
				aspects: Aspects::COLOR,
				levels: 0..1,
				layers: 0..1,
			},
		)}.map_err(|_| "Couldn't create the image view!")?;

		let sampler = unsafe {
			use hal::image::{SamplerDesc, Filter, WrapMode};

			device.create_sampler(&SamplerDesc::new(
				Filter::Nearest,
				WrapMode::Tile,
			))
		}.map_err(|_| "Couldn't create the sampler!")?;

		Ok(LoadedImage {
			image: ManuallyDrop::new(image_ref),
			image_view: ManuallyDrop::new(image_view),
			sampler: ManuallyDrop::new(sampler),
			memory: ManuallyDrop::new(memory)
		})
	}

	/// Properly frees/destroys all the objects in this struct
	/// Dropping without doing this is a bad idea
	pub fn deactivate(self, device: &Device) -> () {
		unsafe {
			use core::ptr::read;
			
			device.destroy_sampler(ManuallyDrop::into_inner(read(&self.sampler)));
			device.destroy_image_view(ManuallyDrop::into_inner(read(&self.image_view)));
			device.destroy_image(ManuallyDrop::into_inner(read(&self.image)));
			device.free_memory(ManuallyDrop::into_inner(read(&self.memory)));
		}
	}
}