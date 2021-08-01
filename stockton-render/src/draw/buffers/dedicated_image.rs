//! A dedicated image. Used for depth buffers.

use crate::draw::texture::PIXEL_SIZE;
use crate::types::*;

use std::mem::ManuallyDrop;

use anyhow::{Context, Result};
use hal::{
    format::{Format, Swizzle},
    image::{SubresourceRange, Usage, Usage as ImgUsage, ViewKind},
    memory,
    memory::Properties,
    MemoryTypeId,
};
use thiserror::Error;

/// Holds an image that's loaded into GPU memory dedicated only to that image, bypassing the memory allocator.
pub struct DedicatedLoadedImage {
    /// The GPU Image handle
    image: ManuallyDrop<ImageT>,

    /// The full view of the image
    pub image_view: ManuallyDrop<ImageViewT>,

    /// The memory backing the image
    memory: ManuallyDrop<MemoryT>,
}

#[derive(Debug, Error)]
pub enum ImageLoadError {
    #[error("No suitable memory type for image memory")]
    NoMemoryTypes,
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
    ) -> Result<DedicatedLoadedImage> {
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
            .context("Error creating image")?;

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
                    .ok_or(ImageLoadError::NoMemoryTypes)?;

                let memory = device
                    .allocate_memory(memory_type_id, requirements.size)
                    .context("Error allocating memory for image")?;

                device
                    .bind_image_memory(&memory, 0, &mut image_ref)
                    .context("Error binding memory to image")?;

                memory
            };

            (memory, image_ref)
        };

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
        .context("Error creating image view")?;

        Ok(DedicatedLoadedImage {
            image: ManuallyDrop::new(image_ref),
            image_view: ManuallyDrop::new(image_view),
            memory: ManuallyDrop::new(memory),
        })
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
