use super::{
    block::LoadedImage, block::TexturesBlock, repo::BLOCK_SIZE, resolver::TextureResolver,
    staging_buffer::StagingBuffer, LoadableImage, PIXEL_SIZE,
};
use crate::types::*;

use anyhow::{Context, Result};
use arrayvec::ArrayVec;
use hal::{
    format::{Aspects, Format, Swizzle},
    image::{
        Filter, SamplerDesc, SubresourceLayers, SubresourceRange, Usage as ImgUsage, ViewKind,
        WrapMode,
    },
    memory::SparseFlags,
    MemoryTypeId,
};
use rendy_memory::{Allocator, Block};
use std::mem::ManuallyDrop;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TextureLoadError {
    #[error("No available resources")]
    NoResources,
}

pub const FORMAT: Format = Format::Rgba8Srgb;
pub const RESOURCES: SubresourceRange = SubresourceRange {
    aspects: Aspects::COLOR,
    level_start: 0,
    level_count: Some(1),
    layer_start: 0,
    layer_count: Some(1),
};
pub const LAYERS: SubresourceLayers = SubresourceLayers {
    aspects: Aspects::COLOR,
    level: 0,
    layers: 0..1,
};

pub struct TextureLoadConfig<R: TextureResolver> {
    pub resolver: R,
    pub filter: Filter,
    pub wrap_mode: WrapMode,
}

pub struct QueuedLoad<B: Block<back::Backend>> {
    pub fence: FenceT,
    pub buf: CommandBufferT,
    pub block: TexturesBlock<B>,
    pub staging_bufs: ArrayVec<[StagingBuffer; BLOCK_SIZE]>,
}

impl<B: Block<back::Backend>> QueuedLoad<B> {
    pub fn dissolve(
        self,
    ) -> (
        (FenceT, CommandBufferT),
        ArrayVec<[StagingBuffer; BLOCK_SIZE]>,
        TexturesBlock<B>,
    ) {
        ((self.fence, self.buf), self.staging_bufs, self.block)
    }
}

pub fn tex_size_info<T: LoadableImage>(img: &T, obcpa: hal::buffer::Offset) -> (usize, usize) {
    let initial_row_size = PIXEL_SIZE * img.width() as usize;
    let row_alignment_mask = obcpa as u32 - 1;

    let row_size = ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;
    let total_size = (row_size * (img.height() as usize)) as u64;
    debug_assert!(row_size as usize >= initial_row_size);

    (row_size, total_size as usize)
}

pub fn create_image_view<T, I>(
    device: &mut DeviceT,
    allocator: &mut T,
    format: Format,
    usage: ImgUsage,
    img: &I,
) -> Result<(T::Block, ImageT)>
where
    T: Allocator<back::Backend>,
    I: LoadableImage,
{
    // Make the image
    let mut image_ref = unsafe {
        use hal::image::{Kind, Tiling, ViewCapabilities};

        device.create_image(
            Kind::D2(img.width(), img.height(), 1, 1),
            1,
            format,
            Tiling::Optimal,
            usage,
            SparseFlags::empty(),
            ViewCapabilities::empty(),
        )
    }
    .context("Error creating image")?;

    // Allocate memory
    let (block, _) = unsafe {
        let requirements = device.get_image_requirements(&image_ref);

        allocator.alloc(device, requirements.size, requirements.alignment)
    }
    .context("Error allocating memory")?;

    unsafe {
        device
            .bind_image_memory(block.memory(), block.range().start, &mut image_ref)
            .context("Error binding memory to image")?;
    }

    Ok((block, image_ref))
}

pub unsafe fn load_image<I: LoadableImage, R: TextureResolver>(
    device: &mut DeviceT,
    staging_allocator: &mut DynamicAllocator,
    tex_allocator: &mut DynamicAllocator,
    staging_memory_type: MemoryTypeId,
    obcpa: u64,
    img_data: I,
    config: &TextureLoadConfig<R>,
) -> Result<(StagingBuffer, LoadedImage<DynamicBlock>)> {
    // Calculate buffer size
    let (row_size, total_size) = tex_size_info(&img_data, obcpa);

    // Create staging buffer
    let mut staging_buffer = StagingBuffer::new(
        device,
        staging_allocator,
        total_size as u64,
        staging_memory_type,
    )
    .context("Error creating staging buffer")?;

    // Write to staging buffer
    let mapped_memory = staging_buffer
        .map_memory(device)
        .context("Error mapping staged memory")?;

    img_data.copy_into(mapped_memory, row_size);

    staging_buffer.unmap_memory(device);

    // Create image
    let (img_mem, img) = create_image_view(
        device,
        tex_allocator,
        FORMAT,
        ImgUsage::SAMPLED | ImgUsage::TRANSFER_DST,
        &img_data,
    )
    .context("Error creating image")?;

    // Create image view
    let img_view = device
        .create_image_view(
            &img,
            ViewKind::D2,
            FORMAT,
            Swizzle::NO,
            ImgUsage::SAMPLED | ImgUsage::TRANSFER_DST,
            RESOURCES,
        )
        .context("Error creating image view")?;

    // Create sampler
    let sampler = device
        .create_sampler(&SamplerDesc::new(config.filter, config.wrap_mode))
        .context("Error creating sampler")?;

    Ok((
        staging_buffer,
        LoadedImage {
            mem: ManuallyDrop::new(img_mem),
            img: ManuallyDrop::new(img),
            img_view: ManuallyDrop::new(img_view),
            sampler: ManuallyDrop::new(sampler),
            row_size,
            height: img_data.height(),
            width: img_data.width(),
        },
    ))
}
