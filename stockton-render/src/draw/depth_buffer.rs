use crate::draw::buffer::create_buffer;
use gfx_hal::{format::Aspects, memory::Properties, MemoryTypeId};
use hal::{
    buffer::Usage as BufUsage,
    format::{Format, Swizzle},
    image::{SubresourceRange, Usage, Usage as ImgUsage, ViewKind},
    memory,
    memory::Segment,
};
use std::{array::IntoIter, convert::TryInto, iter::empty};

use crate::types::*;
use std::mem::ManuallyDrop;

use super::texture::{LoadableImage, PIXEL_SIZE};

/// Holds an image that's loaded into GPU memory dedicated only to that image, bypassing the memory allocator.
pub struct DedicatedLoadedImage {
    /// The GPU Image handle
    image: ManuallyDrop<ImageT>,

    /// The full view of the image
    pub image_view: ManuallyDrop<ImageViewT>,

    /// The memory backing the image
    memory: ManuallyDrop<MemoryT>,
}

impl DedicatedLoadedImage {
    pub fn new(
        device: &mut DeviceT,
        adapter: &Adapter,
        format: Format,
        usage: Usage,
        resources: SubresourceRange,
        width: usize,
        height: usize,
    ) -> Result<DedicatedLoadedImage, &'static str> {
        let (memory, image_ref) = {
            // Round up the size to align properly
            let initial_row_size = PIXEL_SIZE * width;
            let limits = adapter.physical_device.properties().limits;
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
                    memory::SparseFlags::empty(),
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
                            && memory_type.properties.contains(Properties::DEVICE_LOCAL)
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
            device.create_image_view(
                &image_ref,
                ViewKind::D2,
                format,
                Swizzle::NO,
                ImgUsage::DEPTH_STENCIL_ATTACHMENT,
                resources,
            )
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
        device: &mut DeviceT,
        adapter: &Adapter,
        command_queue: &mut QueueT,
        command_pool: &mut CommandPoolT,
    ) -> Result<(), &'static str> {
        let initial_row_size = PIXEL_SIZE * img.width() as usize;
        let limits = adapter.physical_device.properties().limits;
        let row_alignment_mask = limits.optimal_buffer_copy_pitch_alignment as u32 - 1;

        let row_size =
            ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
        let total_size = (row_size * (img.height() as usize)) as u64;
        debug_assert!(row_size as usize >= initial_row_size);

        // Make a staging buffer
        let (staging_buffer, mut staging_memory) = create_buffer(
            device,
            adapter,
            BufUsage::TRANSFER_SRC,
            memory::Properties::CPU_VISIBLE | memory::Properties::COHERENT,
            total_size,
        )
        .map_err(|_| "Couldn't create staging buffer")?;

        // Copy everything into it
        unsafe {
            let mapped_memory: *mut u8 = std::mem::transmute(
                device
                    .map_memory(
                        &mut staging_memory,
                        Segment {
                            offset: 0,
                            size: None,
                        },
                    )
                    .map_err(|_| "Couldn't map buffer memory")?,
            );

            for y in 0..img.height() as usize {
                let dest_base: isize = (y * row_size).try_into().unwrap();
                img.copy_row(y as u32, mapped_memory.offset(dest_base));
            }

            device.unmap_memory(&mut staging_memory);
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
                    level_start: 0,
                    level_count: Some(1),
                    layer_start: 0,
                    layer_count: Some(1),
                },
            };
            buf.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                memory::Dependencies::empty(),
                IntoIter::new([image_barrier]),
            );

            // Copy from buffer to image
            buf.copy_buffer_to_image(
                &staging_buffer,
                &(*self.image),
                Layout::TransferDstOptimal,
                IntoIter::new([BufferImageCopy {
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
                }]),
            );

            // Setup the layout of our image for shaders
            let image_barrier = Barrier::Image {
                states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                    ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                target: &(*self.image),
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    level_start: 0,
                    level_count: Some(1),
                    layer_start: 0,
                    layer_count: Some(1),
                },
            };

            buf.pipeline_barrier(
                PipelineStage::TRANSFER..PipelineStage::FRAGMENT_SHADER,
                memory::Dependencies::empty(),
                IntoIter::new([image_barrier]),
            );

            buf.finish();

            buf
        };

        // Submit our commands and wait for them to finish
        unsafe {
            let mut setup_finished = device.create_fence(false).unwrap();
            command_queue.submit(
                IntoIter::new([&buf]),
                empty(),
                empty(),
                Some(&mut setup_finished),
            );

            device
                .wait_for_fence(&setup_finished, core::u64::MAX)
                .unwrap();
            device.destroy_fence(setup_finished);
        };

        // Clean up temp resources
        unsafe {
            command_pool.free(std::iter::once(buf));

            device.free_memory(staging_memory);
            device.destroy_buffer(staging_buffer);
        }

        Ok(())
    }

    /// Load the given image into a new buffer
    pub fn load_into_new<T: LoadableImage>(
        img: T,
        device: &mut DeviceT,
        adapter: &Adapter,
        command_queue: &mut QueueT,
        command_pool: &mut CommandPoolT,
        format: Format,
        usage: Usage,
    ) -> Result<DedicatedLoadedImage, &'static str> {
        let mut loaded_image = Self::new(
            device,
            adapter,
            format,
            usage | Usage::TRANSFER_DST,
            SubresourceRange {
                aspects: Aspects::COLOR,
                level_start: 0,
                level_count: Some(1),
                layer_start: 0,
                layer_count: Some(1),
            },
            img.width() as usize,
            img.height() as usize,
        )?;
        loaded_image.load(img, device, adapter, command_queue, command_pool)?;

        Ok(loaded_image)
    }

    /// Properly frees/destroys all the objects in this struct
    /// Dropping without doing this is a bad idea
    pub fn deactivate(self, device: &mut DeviceT) {
        unsafe {
            use core::ptr::read;

            device.destroy_image_view(ManuallyDrop::into_inner(read(&self.image_view)));
            device.destroy_image(ManuallyDrop::into_inner(read(&self.image)));
            device.free_memory(ManuallyDrop::into_inner(read(&self.memory)));
        }
    }
}
