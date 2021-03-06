/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use core::{
    mem::{size_of, ManuallyDrop},
    ptr::copy_nonoverlapping,
};
use hal::{
    buffer::Usage as BufUsage,
    format::{Aspects, Format, Swizzle},
    image::{SubresourceRange, Usage as ImgUsage, ViewKind},
    memory::{Dependencies as MemDependencies, Properties as MemProperties},
    prelude::*,
    queue::Submission,
    MemoryTypeId,
};
use image::RgbaImage;
use rendy_memory::{Allocator, Block};
use std::{convert::TryInto, iter::once};

use crate::draw::buffer::create_buffer;
use crate::types::*;

/// The size of each pixel in an image
const PIXEL_SIZE: usize = size_of::<u8>() * 4;

/// An object that can be loaded as an image into GPU memory
pub trait LoadableImage {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn copy_row(&self, y: u32, ptr: *mut u8);
}

impl LoadableImage for RgbaImage {
    fn width(&self) -> u32 {
        self.width()
    }

    fn height(&self) -> u32 {
        self.height()
    }

    fn copy_row(&self, y: u32, ptr: *mut u8) {
        let row_size_bytes = self.width() as usize * PIXEL_SIZE;
        let raw: &Vec<u8> = self.as_raw();
        let row = &raw[y as usize * row_size_bytes..(y as usize + 1) * row_size_bytes];

        unsafe {
            copy_nonoverlapping(row.as_ptr(), ptr, row.len());
        }
    }
}

/// Holds an image that's loaded into GPU memory and can be sampled from
pub struct LoadedImage {
    /// The GPU Image handle
    image: ManuallyDrop<Image>,

    /// The full view of the image
    pub image_view: ManuallyDrop<ImageView>,

    /// The memory backing the image
    memory: ManuallyDrop<DynamicBlock>,
}

pub fn create_image_view(
    device: &mut Device,
    adapter: &Adapter,
    allocator: &mut DynamicAllocator,
    format: Format,
    usage: ImgUsage,
    width: usize,
    height: usize,
) -> Result<(DynamicBlock, Image), &'static str> {
    // Round up the size to align properly
    let initial_row_size = PIXEL_SIZE * width;
    let limits = adapter.physical_device.limits();
    let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;

    let row_size = ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
    debug_assert!(row_size as usize >= initial_row_size);

    // Make the image
    let mut image_ref = unsafe {
        use hal::image::{Kind, Tiling, ViewCapabilities};

        device.create_image(
            Kind::D2(width as u32, height as u32, 1, 1),
            1,
            format,
            Tiling::Optimal,
            usage,
            ViewCapabilities::empty(),
        )
    }
    .map_err(|_| "Couldn't create image")?;

    // Allocate memory
    let (block, _) = unsafe {
        let requirements = device.get_image_requirements(&image_ref);

        allocator.alloc(device, requirements.size, requirements.alignment)
    }
    .map_err(|_| "Out of memory")?;

    unsafe {
        device
            .bind_image_memory(&block.memory(), block.range().start, &mut image_ref)
            .map_err(|_| "Couldn't bind memory to image")?;
    }

    Ok((block, image_ref))
}

impl LoadedImage {
    pub fn new(
        device: &mut Device,
        adapter: &Adapter,
        allocator: &mut DynamicAllocator,
        format: Format,
        usage: ImgUsage,
        resources: SubresourceRange,
        width: usize,
        height: usize,
    ) -> Result<LoadedImage, &'static str> {
        let (memory, image_ref) =
            create_image_view(device, adapter, allocator, format, usage, width, height)?;

        // Create ImageView and sampler
        let image_view = unsafe {
            device.create_image_view(&image_ref, ViewKind::D2, format, Swizzle::NO, resources)
        }
        .map_err(|_| "Couldn't create the image view!")?;

        Ok(LoadedImage {
            image: ManuallyDrop::new(image_ref),
            image_view: ManuallyDrop::new(image_view),
            memory: ManuallyDrop::new(memory),
        })
    }

    /// Load the given image
    pub fn load<T: LoadableImage>(
        &mut self,
        img: T,
        device: &mut Device,
        adapter: &Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
    ) -> Result<(), &'static str> {
        let initial_row_size = PIXEL_SIZE * img.width() as usize;
        let limits = adapter.physical_device.limits();
        let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;

        let row_size =
            ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
        let total_size = (row_size * (img.height() as usize)) as u64;
        debug_assert!(row_size as usize >= initial_row_size);

        // Make a staging buffer
        let (staging_buffer, staging_memory) = create_buffer(
            device,
            adapter,
            BufUsage::TRANSFER_SRC,
            MemProperties::CPU_VISIBLE | MemProperties::COHERENT,
            total_size,
        )
        .map_err(|_| "Couldn't create staging buffer")?;

        // Copy everything into it
        unsafe {
            let mapped_memory: *mut u8 = std::mem::transmute(
                device
                    .map_memory(&staging_memory, 0..total_size)
                    .map_err(|_| "Couldn't map buffer memory")?,
            );

            for y in 0..img.height() as usize {
                let dest_base: isize = (y * row_size).try_into().unwrap();
                img.copy_row(y as u32, mapped_memory.offset(dest_base));
            }

            device.unmap_memory(&staging_memory);
        }

        // Copy from staging to image memory
        let buf = unsafe {
            use hal::command::{BufferImageCopy, CommandBufferFlags};
            use hal::image::{Access, Extent, Layout, Offset, SubresourceLayers};
            use hal::memory::Barrier;
            use hal::pso::PipelineStage;

            // Get a command buffer
            let mut buf = command_pool.allocate_one(hal::command::Level::Primary);
            buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            // Setup the layout of our image for copying
            let image_barrier = Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                target: &(*self.image),
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
            buf.copy_buffer_to_image(
                &staging_buffer,
                &(*self.image),
                Layout::TransferDstOptimal,
                &[BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: (row_size / PIXEL_SIZE) as u32,
                    buffer_height: img.height(),
                    image_layers: SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: Offset { x: 0, y: 0, z: 0 },
                    image_extent: Extent {
                        width: img.width(),
                        height: img.height(),
                        depth: 1,
                    },
                }],
            );

            // Setup the layout of our image for shaders
            let image_barrier = Barrier::Image {
                states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                    ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                target: &(*self.image),
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
            command_queue.submit::<_, _, Semaphore, _, _>(
                Submission {
                    command_buffers: &[&buf],
                    wait_semaphores: std::iter::empty::<_>(),
                    signal_semaphores: std::iter::empty::<_>(),
                },
                Some(&setup_finished),
            );

            device
                .wait_for_fence(&setup_finished, core::u64::MAX)
                .unwrap();
            device.destroy_fence(setup_finished);
        };

        // Clean up temp resources
        unsafe {
            command_pool.free(once(buf));

            device.free_memory(staging_memory);
            device.destroy_buffer(staging_buffer);
        }

        Ok(())
    }

    /// Properly frees/destroys all the objects in this struct
    /// Dropping without doing this is a bad idea
    pub fn deactivate(self, device: &mut Device, allocator: &mut DynamicAllocator) {
        unsafe {
            use core::ptr::read;

            device.destroy_image_view(ManuallyDrop::into_inner(read(&self.image_view)));
            device.destroy_image(ManuallyDrop::into_inner(read(&self.image)));
            allocator.free(device, ManuallyDrop::into_inner(read(&self.memory)));
        }
    }
}

pub struct SampledImage {
    pub image: ManuallyDrop<LoadedImage>,
    pub sampler: ManuallyDrop<Sampler>,
}

impl SampledImage {
    pub fn new(
        device: &mut Device,
        adapter: &Adapter,
        allocator: &mut DynamicAllocator,
        format: Format,
        usage: ImgUsage,
        width: usize,
        height: usize,
    ) -> Result<SampledImage, &'static str> {
        let image = LoadedImage::new(
            device,
            adapter,
            allocator,
            format,
            usage | ImgUsage::SAMPLED,
            SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            },
            width,
            height,
        )?;

        let sampler = unsafe {
            use hal::image::{Filter, SamplerDesc, WrapMode};

            device.create_sampler(&SamplerDesc::new(Filter::Nearest, WrapMode::Tile))
        }
        .map_err(|_| "Couldn't create the sampler!")?;

        Ok(SampledImage {
            image: ManuallyDrop::new(image),
            sampler: ManuallyDrop::new(sampler),
        })
    }

    pub fn load_into_new<T: LoadableImage>(
        img: T,
        device: &mut Device,
        adapter: &Adapter,
        allocator: &mut DynamicAllocator,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
        format: Format,
        usage: ImgUsage,
    ) -> Result<SampledImage, &'static str> {
        let mut sampled_image = SampledImage::new(
            device,
            adapter,
            allocator,
            format,
            usage | ImgUsage::TRANSFER_DST,
            img.width() as usize,
            img.height() as usize,
        )?;
        sampled_image
            .image
            .load(img, device, adapter, command_queue, command_pool)?;

        Ok(sampled_image)
    }

    pub fn deactivate(self, device: &mut Device, allocator: &mut DynamicAllocator) {
        unsafe {
            use core::ptr::read;

            device.destroy_sampler(ManuallyDrop::into_inner(read(&self.sampler)));

            ManuallyDrop::into_inner(read(&self.image)).deactivate(device, allocator);
        }
    }
}

/// Holds an image that's loaded into GPU memory dedicated only to that image, bypassing the memory allocator.
pub struct DedicatedLoadedImage {
    /// The GPU Image handle
    image: ManuallyDrop<Image>,

    /// The full view of the image
    pub image_view: ManuallyDrop<ImageView>,

    /// The memory backing the image
    memory: ManuallyDrop<Memory>,
}

impl DedicatedLoadedImage {
    pub fn new(
        device: &mut Device,
        adapter: &Adapter,
        format: Format,
        usage: ImgUsage,
        resources: SubresourceRange,
        width: usize,
        height: usize,
    ) -> Result<DedicatedLoadedImage, &'static str> {
        let (memory, image_ref) = {
            // Round up the size to align properly
            let initial_row_size = PIXEL_SIZE * width;
            let limits = adapter.physical_device.limits();
            let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;

            let row_size =
                ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
            debug_assert!(row_size as usize >= initial_row_size);

            // Make the image
            let mut image_ref = unsafe {
                use hal::image::{Kind, Tiling, ViewCapabilities};

                device.create_image(
                    Kind::D2(width as u32, height as u32, 1, 1),
                    1,
                    format,
                    Tiling::Optimal,
                    usage,
                    ViewCapabilities::empty(),
                )
            }
            .map_err(|_| "Couldn't create image")?;

            // Allocate memory

            // Allocate memory
            let memory = unsafe {
                let requirements = device.get_image_requirements(&image_ref);

                let memory_type_id = adapter
                    .physical_device
                    .memory_properties()
                    .memory_types
                    .iter()
                    .enumerate()
                    .find(|&(id, memory_type)| {
                        requirements.type_mask & (1 << id) != 0
                            && memory_type.properties.contains(MemProperties::DEVICE_LOCAL)
                    })
                    .map(|(id, _)| MemoryTypeId(id))
                    .ok_or("Couldn't find a memory type for image memory")?;

                let memory = device
                    .allocate_memory(memory_type_id, requirements.size)
                    .map_err(|_| "Couldn't allocate image memory")?;

                device
                    .bind_image_memory(&memory, 0, &mut image_ref)
                    .map_err(|_| "Couldn't bind memory to image")?;

                Ok(memory)
            }?;

            Ok((memory, image_ref))
        }?;

        // Create ImageView and sampler
        let image_view = unsafe {
            device.create_image_view(&image_ref, ViewKind::D2, format, Swizzle::NO, resources)
        }
        .map_err(|_| "Couldn't create the image view!")?;

        Ok(DedicatedLoadedImage {
            image: ManuallyDrop::new(image_ref),
            image_view: ManuallyDrop::new(image_view),
            memory: ManuallyDrop::new(memory),
        })
    }

    /// Load the given image
    pub fn load<T: LoadableImage>(
        &mut self,
        img: T,
        device: &mut Device,
        adapter: &Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
    ) -> Result<(), &'static str> {
        let initial_row_size = PIXEL_SIZE * img.width() as usize;
        let limits = adapter.physical_device.limits();
        let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;

        let row_size =
            ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
        let total_size = (row_size * (img.height() as usize)) as u64;
        debug_assert!(row_size as usize >= initial_row_size);

        // Make a staging buffer
        let (staging_buffer, staging_memory) = create_buffer(
            device,
            adapter,
            BufUsage::TRANSFER_SRC,
            MemProperties::CPU_VISIBLE | MemProperties::COHERENT,
            total_size,
        )
        .map_err(|_| "Couldn't create staging buffer")?;

        // Copy everything into it
        unsafe {
            let mapped_memory: *mut u8 = std::mem::transmute(
                device
                    .map_memory(&staging_memory, 0..total_size)
                    .map_err(|_| "Couldn't map buffer memory")?,
            );

            for y in 0..img.height() as usize {
                let dest_base: isize = (y * row_size).try_into().unwrap();
                img.copy_row(y as u32, mapped_memory.offset(dest_base));
            }

            device.unmap_memory(&staging_memory);
        }

        // Copy from staging to image memory
        let buf = unsafe {
            use hal::command::{BufferImageCopy, CommandBufferFlags};
            use hal::image::{Access, Extent, Layout, Offset, SubresourceLayers};
            use hal::memory::Barrier;
            use hal::pso::PipelineStage;

            // Get a command buffer
            let mut buf = command_pool.allocate_one(hal::command::Level::Primary);
            buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

            // Setup the layout of our image for copying
            let image_barrier = Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                target: &(*self.image),
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
            buf.copy_buffer_to_image(
                &staging_buffer,
                &(*self.image),
                Layout::TransferDstOptimal,
                &[BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: (row_size / PIXEL_SIZE) as u32,
                    buffer_height: img.height(),
                    image_layers: SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: Offset { x: 0, y: 0, z: 0 },
                    image_extent: Extent {
                        width: img.width(),
                        height: img.height(),
                        depth: 1,
                    },
                }],
            );

            // Setup the layout of our image for shaders
            let image_barrier = Barrier::Image {
                states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                    ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                target: &(*self.image),
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
            command_queue.submit::<_, _, Semaphore, _, _>(
                Submission {
                    command_buffers: &[&buf],
                    wait_semaphores: std::iter::empty::<_>(),
                    signal_semaphores: std::iter::empty::<_>(),
                },
                Some(&setup_finished),
            );

            device
                .wait_for_fence(&setup_finished, core::u64::MAX)
                .unwrap();
            device.destroy_fence(setup_finished);
        };

        // Clean up temp resources
        unsafe {
            command_pool.free(once(buf));

            device.free_memory(staging_memory);
            device.destroy_buffer(staging_buffer);
        }

        Ok(())
    }

    /// Load the given image into a new buffer
    pub fn load_into_new<T: LoadableImage>(
        img: T,
        device: &mut Device,
        adapter: &Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
        format: Format,
        usage: ImgUsage,
    ) -> Result<DedicatedLoadedImage, &'static str> {
        let mut loaded_image = Self::new(
            device,
            adapter,
            format,
            usage | ImgUsage::TRANSFER_DST,
            SubresourceRange {
                aspects: Aspects::COLOR,
                levels: 0..1,
                layers: 0..1,
            },
            img.width() as usize,
            img.height() as usize,
        )?;
        loaded_image.load(img, device, adapter, command_queue, command_pool)?;

        Ok(loaded_image)
    }

    /// Properly frees/destroys all the objects in this struct
    /// Dropping without doing this is a bad idea
    pub fn deactivate(self, device: &mut Device) {
        unsafe {
            use core::ptr::read;

            device.destroy_image_view(ManuallyDrop::into_inner(read(&self.image_view)));
            device.destroy_image(ManuallyDrop::into_inner(read(&self.image)));
            device.free_memory(ManuallyDrop::into_inner(read(&self.memory)));
        }
    }
}
