use super::{
    block::LoadedImage, block::TexturesBlock, loader::TextureLoader, repo::BLOCK_SIZE,
    resolver::TextureResolver, staging_buffer::StagingBuffer, LoadableImage, PIXEL_SIZE,
};
use crate::{error::LockPoisoned, types::*};
use stockton_levels::prelude::*;

use anyhow::{Context, Result};
use arrayvec::ArrayVec;
use hal::{
    command::{BufferImageCopy, CommandBufferFlags},
    format::{Aspects, Format, Swizzle},
    image::{
        Access, Extent, Filter, Layout, Offset, SamplerDesc, SubresourceLayers, SubresourceRange,
        Usage as ImgUsage, ViewKind, WrapMode,
    },
    memory::{Barrier, Dependencies, SparseFlags},
    pso::{Descriptor, DescriptorSetWrite, ImageDescriptorType, PipelineStage, ShaderStageFlags},
    MemoryTypeId,
};
use image::{Rgba, RgbaImage};
use rendy_descriptor::{DescriptorRanges, DescriptorSetLayoutBinding, DescriptorType};
use rendy_memory::{Allocator, Block};
use std::{
    array::IntoIter,
    iter::{empty, once},
    mem::ManuallyDrop,
    sync::{Arc, RwLock},
};
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TextureLoadError {
    #[error("No available resources")]
    NoResources,

    #[error("Texture could not be resolved")]
    ResolveFailed(usize),
}

const FORMAT: Format = Format::Rgba8Srgb;
const RESOURCES: SubresourceRange = SubresourceRange {
    aspects: Aspects::COLOR,
    level_start: 0,
    level_count: Some(1),
    layer_start: 0,
    layer_count: Some(1),
};
const LAYERS: SubresourceLayers = SubresourceLayers {
    aspects: Aspects::COLOR,
    level: 0,
    layers: 0..1,
};

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

impl<'a, T: HasTextures, R: TextureResolver<I>, I: LoadableImage> TextureLoader<T, R, I> {
    pub(crate) unsafe fn attempt_queue_load(
        &mut self,
        block_ref: usize,
    ) -> Result<QueuedLoad<DynamicBlock>> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        let textures = self
            .textures
            .read()
            .map_err(|_| LockPoisoned::Map)
            .context("Error getting map lock")?;

        // Get assets to use
        let (mut fence, mut buf) = self
            .buffers
            .pop_front()
            .ok_or(TextureLoadError::NoResources)
            .context("Error getting resources to use")?;

        // Create descriptor set
        let mut descriptor_set = {
            let mut v: ArrayVec<[RDescriptorSet; 1]> = ArrayVec::new();
            self.descriptor_allocator
                .allocate(
                    &device,
                    &*self.ds_layout.read().unwrap(),
                    DescriptorRanges::from_bindings(&[
                        DescriptorSetLayoutBinding {
                            binding: 0,
                            ty: DescriptorType::Image {
                                ty: ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: BLOCK_SIZE,
                            stage_flags: ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                        DescriptorSetLayoutBinding {
                            binding: 1,
                            ty: DescriptorType::Sampler,
                            count: BLOCK_SIZE,
                            stage_flags: ShaderStageFlags::FRAGMENT,
                            immutable_samplers: false,
                        },
                    ]),
                    1,
                    &mut v,
                )
                .context("Error creating descriptor set")?;

            v.pop().unwrap()
        };

        // Get a command buffer
        buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        let mut imgs: ArrayVec<[_; BLOCK_SIZE]> = ArrayVec::new();
        let mut staging_bufs: ArrayVec<[_; BLOCK_SIZE]> = ArrayVec::new();

        // For each texture in block
        for tex_idx in (block_ref * BLOCK_SIZE)..(block_ref + 1) * BLOCK_SIZE {
            // Get texture and Resolve image
            let tex = textures.get_texture(tex_idx as u32);
            if tex.is_none() {
                // Write a blank descriptor
                device.write_descriptor_set(DescriptorSetWrite {
                    set: descriptor_set.raw_mut(),
                    binding: 0,
                    array_offset: tex_idx % BLOCK_SIZE,
                    descriptors: once(Descriptor::Image(
                        &*self.blank_image.img_view,
                        Layout::ShaderReadOnlyOptimal,
                    )),
                });
                device.write_descriptor_set(DescriptorSetWrite {
                    set: descriptor_set.raw_mut(),
                    binding: 1,
                    array_offset: tex_idx % BLOCK_SIZE,
                    descriptors: once(Descriptor::Sampler(&*self.blank_image.sampler)),
                });

                continue;
            }

            let tex = tex.unwrap();

            let img_data = self
                .resolver
                .resolve(tex)
                .ok_or(TextureLoadError::ResolveFailed(tex_idx))?;
            let array_offset = tex_idx % BLOCK_SIZE;

            let (staging_buffer, img) = load_image(
                &mut device,
                &mut self.staging_allocator,
                &mut self.tex_allocator,
                self.staging_memory_type,
                self.optimal_buffer_copy_pitch_alignment,
                img_data,
            )?;

            // Write to descriptor set
            {
                device.write_descriptor_set(DescriptorSetWrite {
                    set: descriptor_set.raw_mut(),
                    binding: 0,
                    array_offset,
                    descriptors: once(Descriptor::Image(
                        &*img.img_view,
                        Layout::ShaderReadOnlyOptimal,
                    )),
                });
                device.write_descriptor_set(DescriptorSetWrite {
                    set: descriptor_set.raw_mut(),
                    binding: 1,
                    array_offset,
                    descriptors: once(Descriptor::Sampler(&*img.sampler)),
                });
            }

            imgs.push(img);

            staging_bufs.push(staging_buffer);
        }

        // Add start pipeline barrier
        buf.pipeline_barrier(
            PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
            Dependencies::empty(),
            imgs.iter().map(|li| Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                target: &*li.img,
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    level_start: 0,
                    level_count: None,
                    layer_start: 0,
                    layer_count: None,
                },
            }),
        );

        // Record copy commands
        for (li, sb) in imgs.iter().zip(staging_bufs.iter()) {
            buf.copy_buffer_to_image(
                &*sb.buf,
                &*li.img,
                Layout::TransferDstOptimal,
                once(BufferImageCopy {
                    buffer_offset: 0,
                    buffer_width: (li.row_size / super::PIXEL_SIZE) as u32,
                    buffer_height: li.height,
                    image_layers: SubresourceLayers {
                        aspects: Aspects::COLOR,
                        level: 0,
                        layers: 0..1,
                    },
                    image_offset: Offset { x: 0, y: 0, z: 0 },
                    image_extent: gfx_hal::image::Extent {
                        width: li.width,
                        height: li.height,
                        depth: 1,
                    },
                }),
            );
        }
        buf.pipeline_barrier(
            PipelineStage::TRANSFER..PipelineStage::BOTTOM_OF_PIPE,
            Dependencies::empty(),
            imgs.iter().map(|li| Barrier::Image {
                states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                    ..(Access::empty(), Layout::ShaderReadOnlyOptimal),
                target: &*li.img,
                families: None,
                range: RESOURCES,
            }),
        );

        buf.finish();

        // Submit command buffer
        {
            let mut queue = self.queue.write().map_err(|_| LockPoisoned::Queue)?;

            queue.submit(IntoIter::new([&buf]), empty(), empty(), Some(&mut fence));
        }

        Ok(QueuedLoad {
            staging_bufs,
            fence,
            buf,
            block: TexturesBlock {
                id: block_ref,
                imgs,
                descriptor_set: ManuallyDrop::new(descriptor_set),
            },
        })
    }

    pub(crate) unsafe fn get_blank_image(
        device: &mut DeviceT,
        buf: &mut CommandBufferT,
        queue_lock: &Arc<RwLock<QueueT>>,
        staging_allocator: &mut DynamicAllocator,
        tex_allocator: &mut DynamicAllocator,
        staging_memory_type: MemoryTypeId,
        obcpa: u64,
    ) -> Result<LoadedImage<DynamicBlock>> {
        let img_data = RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 0]));

        let height = img_data.height();
        let width = img_data.width();
        let row_alignment_mask = obcpa as u32 - 1;
        let initial_row_size = PIXEL_SIZE * img_data.width() as usize;
        let row_size =
            ((initial_row_size as u32 + row_alignment_mask) & !row_alignment_mask) as usize;

        let (staging_buffer, img) = load_image(
            device,
            staging_allocator,
            tex_allocator,
            staging_memory_type,
            obcpa,
            img_data,
        )?;

        buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        buf.pipeline_barrier(
            PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
            Dependencies::empty(),
            once(Barrier::Image {
                states: (Access::empty(), Layout::Undefined)
                    ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                target: &*img.img,
                families: None,
                range: SubresourceRange {
                    aspects: Aspects::COLOR,
                    level_start: 0,
                    level_count: None,
                    layer_start: 0,
                    layer_count: None,
                },
            }),
        );
        buf.copy_buffer_to_image(
            &*staging_buffer.buf,
            &*img.img,
            Layout::TransferDstOptimal,
            once(BufferImageCopy {
                buffer_offset: 0,
                buffer_width: (row_size / super::PIXEL_SIZE) as u32,
                buffer_height: height,
                image_layers: LAYERS,
                image_offset: Offset { x: 0, y: 0, z: 0 },
                image_extent: Extent {
                    width: width,
                    height: height,
                    depth: 1,
                },
            }),
        );

        buf.pipeline_barrier(
            PipelineStage::TRANSFER..PipelineStage::BOTTOM_OF_PIPE,
            Dependencies::empty(),
            once(Barrier::Image {
                states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                    ..(Access::empty(), Layout::ShaderReadOnlyOptimal),
                target: &*img.img,
                families: None,
                range: RESOURCES,
            }),
        );
        buf.finish();

        let mut fence = device.create_fence(false).context("Error creating fence")?;

        {
            let mut queue = queue_lock.write().map_err(|_| LockPoisoned::Queue)?;

            queue.submit(
                IntoIter::new([buf as &CommandBufferT]),
                empty(),
                empty(),
                Some(&mut fence),
            );
        }

        device
            .wait_for_fence(&fence, std::u64::MAX)
            .context("Error waiting for copy")?;

        device.destroy_fence(fence);

        staging_buffer.deactivate(device, staging_allocator);

        Ok(img)
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

fn create_image_view<T, I>(
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
            .bind_image_memory(&block.memory(), block.range().start, &mut image_ref)
            .context("Error binding memory to image")?;
    }

    Ok((block, image_ref))
}

unsafe fn load_image<I: LoadableImage>(
    device: &mut DeviceT,
    staging_allocator: &mut DynamicAllocator,
    tex_allocator: &mut DynamicAllocator,
    staging_memory_type: MemoryTypeId,
    obcpa: u64,
    img_data: I,
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
        .create_sampler(&SamplerDesc::new(Filter::Nearest, WrapMode::Tile))
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
