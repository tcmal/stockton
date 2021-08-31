use std::sync::{Arc, RwLock};

use super::{block::TexturesBlock, repo::BLOCK_SIZE, TextureResolver, LoadableImage};
use crate::{
    buffers::{
        image::{ImageSpec, SampledImage, COLOR_RESOURCES},
        staging::StagingBuffer,
    },
    error::LockPoisoned,
    mem::{Block, MappableBlock, MemoryPool},
    types::*,
};

use anyhow::{Context, Result};
use arrayvec::ArrayVec;
use hal::{
    format::{Aspects, Format},
    image::{
        Filter, SamplerDesc, SubresourceLayers, SubresourceRange, Usage as ImgUsage, WrapMode,
    },
};
use thiserror::Error;

/// The format used by the texture repo
// TODO: This should be customisable.
pub const FORMAT: Format = Format::Rgba8Srgb;

/// The resources used by each texture. ie one colour aspect
pub const RESOURCES: SubresourceRange = SubresourceRange {
    aspects: Aspects::COLOR,
    level_start: 0,
    level_count: Some(1),
    layer_start: 0,
    layer_count: Some(1),
};

/// The layers used by each texture. ie one colour layer
pub const LAYERS: SubresourceLayers = SubresourceLayers {
    aspects: Aspects::COLOR,
    level: 0,
    layers: 0..1,
};

/// Configuration required to load a texture
pub struct TextureLoadConfig<R: TextureResolver> {
    /// The resolver to use
    pub resolver: R,

    /// How to sample the image
    pub filter: Filter,

    /// How to deal with texture coordinates outside the image.
    pub wrap_mode: WrapMode,
}

/// A texture load that has been queued, and is finished when the fence triggers.
pub struct QueuedLoad<TP: MemoryPool, SP: MemoryPool> {
    pub fence: FenceT,
    pub buf: CommandBufferT,
    pub block: TexturesBlock<TP>,
    pub staging_bufs: ArrayVec<[StagingBuffer<SP>; BLOCK_SIZE]>,
}

/// Create a SampledImage for the given LoadableImage, and load the image data into a StagingBuffer
/// Note that this doesn't queue up transferring from the buffer to the image.
pub unsafe fn load_image<I, R, SP, TP>(
    device: &mut DeviceT,
    staging_allocator: &Arc<RwLock<SP>>,
    tex_allocator: &Arc<RwLock<TP>>,
    obcpa: u32,
    img_data: I,
    config: &TextureLoadConfig<R>,
) -> Result<(StagingBuffer<SP>, SampledImage<TP>)>
where
    I: LoadableImage,
    R: TextureResolver,
    SP: MemoryPool,
    TP: MemoryPool,
    SP::Block: MappableBlock,
{
    // Create sampled image
    let sampled_image = {
        let mut tex_allocator = tex_allocator
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        SampledImage::from_device_allocator(
            device,
            &mut *tex_allocator,
            obcpa as u32,
            &ImageSpec {
                width: img_data.width(),
                height: img_data.height(),
                format: FORMAT,
                usage: ImgUsage::TRANSFER_DST | ImgUsage::SAMPLED,
                resources: COLOR_RESOURCES,
            },
            &SamplerDesc::new(config.filter, config.wrap_mode),
        )?
    };

    // Create staging buffer
    let total_size = sampled_image.bound_image().mem().size();

    let mut staging_buffer = {
        let mut staging_allocator = staging_allocator
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        StagingBuffer::from_device_pool(device, &mut *staging_allocator, total_size as u64)
            .context("Error creating staging buffer")?
    };

    // Write to staging buffer
    let mapped_memory = staging_buffer
        .map(device, 0..total_size)
        .context("Error mapping staged memory")?;

    img_data.copy_into(mapped_memory, sampled_image.row_size() as usize);

    staging_buffer.unmap(device)?;

    Ok((staging_buffer, sampled_image))
}

/// Errors that can be encountered when loading a texture.
#[derive(Error, Debug)]
pub enum TextureLoadError {
    #[error("No available resources")]
    NoResources,
}
