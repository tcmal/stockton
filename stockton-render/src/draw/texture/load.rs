use super::{
    block::LoadedImage, block::TexturesBlock, loader::TextureLoader, repo::BLOCK_SIZE,
    resolver::TextureResolver, staging_buffer::StagingBuffer, LoadableImage, PIXEL_SIZE,
};
use crate::types::*;
use stockton_levels::prelude::*;

use anyhow::{Context, Result};
use arrayvec::ArrayVec;
use hal::{
    command::{BufferImageCopy, CommandBufferFlags},
    format::{Aspects, Format, Swizzle},
    image::{
        Extent, Filter, Layout, Offset, SamplerDesc, SubresourceLayers, SubresourceRange,
        Usage as ImgUsage, ViewKind, WrapMode,
    },
    memory::{Barrier, Dependencies},
    prelude::*,
    pso::{Descriptor, DescriptorSetWrite, PipelineStage, ShaderStageFlags},
    queue::Submission,
};
use rendy_descriptor::{DescriptorRanges, DescriptorSetLayoutBinding, DescriptorType};
use rendy_memory::{Allocator, Block};
use std::mem::ManuallyDrop;
use thiserror::Error;

#[derive(Error, Debug)]
pub enum TextureLoadError {
    #[error("No available resources")]
    NoResources,

    #[error("Texture is not in map")]
    NotInMap(usize),

    #[error("Texture could not be resolved")]
    ResolveFailed(usize),
}

pub struct QueuedLoad<B: Block<back::Backend>> {
    pub fence: Fence,
    pub buf: CommandBuffer,
    pub block: TexturesBlock<B>,
    pub staging_bufs: ArrayVec<[StagingBuffer; BLOCK_SIZE]>,
}

impl<B: Block<back::Backend>> QueuedLoad<B> {
    pub fn dissolve(
        self,
    ) -> (
        (Fence, CommandBuffer),
        ArrayVec<[StagingBuffer; BLOCK_SIZE]>,
        TexturesBlock<B>,
    ) {
        ((self.fence, self.buf), self.staging_bufs, self.block)
    }
}

impl<'a, T: HasTextures, R: TextureResolver<I>, I: LoadableImage> TextureLoader<T, R, I> {
    const FORMAT: Format = Format::Rgba8Srgb;
    const RESOURCES: SubresourceRange = SubresourceRange {
        aspects: Aspects::COLOR,
        levels: 0..1,
        layers: 0..1,
    };
    const LAYERS: SubresourceLayers = SubresourceLayers {
        aspects: Aspects::COLOR,
        level: 0,
        layers: 0..1,
    };

    pub(crate) unsafe fn attempt_queue_load(
        &mut self,
        block_ref: usize,
    ) -> Result<QueuedLoad<DynamicBlock>> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        let textures = self.textures.read().unwrap();

        // Get assets to use
        let (fence, mut buf) = self
            .buffers
            .pop_front()
            .ok_or(TextureLoadError::NoResources)
            .context("Error getting resources to use")?;

        // Create descriptor set
        let descriptor_set = {
            let mut v: ArrayVec<[RDescriptorSet; 1]> = ArrayVec::new();
            self.descriptor_allocator
                .allocate(
                    &device,
                    &*self.ds_layout.read().unwrap(),
                    DescriptorRanges::from_bindings(&[
                        DescriptorSetLayoutBinding {
                            binding: 0,
                            ty: DescriptorType::SampledImage,
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
                .map_err::<HalErrorWrapper, _>(|e| e.into())
                .context("Error creating descriptor set")?;

            v.pop().unwrap()
        };

        // Get a command buffer
        buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);

        let mut copy_cmds: ArrayVec<[_; BLOCK_SIZE]> = ArrayVec::new();
        let mut imgs: ArrayVec<[_; BLOCK_SIZE]> = ArrayVec::new();
        let mut staging_bufs: ArrayVec<[_; BLOCK_SIZE]> = ArrayVec::new();

        // For each texture in block
        for tex_idx in (block_ref * BLOCK_SIZE)..(block_ref + 1) * BLOCK_SIZE {
            // Get texture and Resolve image
            let tex = textures.get_texture(tex_idx as u32);
            if tex.is_none() {
                break; // Past the end
                       // TODO: We should actually write blank descriptors
            }
            let tex = tex.ok_or(TextureLoadError::NotInMap(tex_idx))?;

            let img_data = self
                .resolver
                .resolve(tex)
                .ok_or(TextureLoadError::ResolveFailed(tex_idx))?;

            // Calculate buffer size
            let (row_size, total_size) =
                tex_size_info(&img_data, self.optimal_buffer_copy_pitch_alignment);

            // Create staging buffer
            let mut staging_buffer = StagingBuffer::new(
                &mut device,
                &mut self.staging_allocator,
                total_size as u64,
                self.staging_memory_type,
            )
            .context("Error creating staging buffer")?;

            // Write to staging buffer
            let mapped_memory = staging_buffer
                .map_memory(&mut device)
                .map_err::<HalErrorWrapper, _>(|e| e.into())
                .context("Error mapping staged memory")?;

            img_data.copy_into(mapped_memory, row_size);

            staging_buffer.unmap_memory(&mut device);

            // Create image
            let (img_mem, img) = create_image_view(
                &mut device,
                &mut *self.tex_allocator,
                Self::FORMAT,
                ImgUsage::SAMPLED,
                &img_data,
            )
            .context("Error creating image")?;

            // Create image view
            let img_view = device
                .create_image_view(
                    &img,
                    ViewKind::D2,
                    Self::FORMAT,
                    Swizzle::NO,
                    Self::RESOURCES,
                )
                .map_err::<HalErrorWrapper, _>(|e| e.into())
                .context("Error creating image view")?;

            // Queue copy from buffer to image
            copy_cmds.push(BufferImageCopy {
                buffer_offset: 0,
                buffer_width: (row_size / super::PIXEL_SIZE) as u32,
                buffer_height: img_data.height(),
                image_layers: Self::LAYERS,
                image_offset: Offset { x: 0, y: 0, z: 0 },
                image_extent: Extent {
                    width: img_data.width(),
                    height: img_data.height(),
                    depth: 1,
                },
            });

            // Create sampler
            let sampler = device
                .create_sampler(&SamplerDesc::new(Filter::Nearest, WrapMode::Tile))
                .map_err::<HalErrorWrapper, _>(|e| e.into())
                .context("Error creating sampler")?;

            // Write to descriptor set
            {
                device.write_descriptor_sets(vec![
                    DescriptorSetWrite {
                        set: descriptor_set.raw(),
                        binding: 0,
                        array_offset: tex_idx % BLOCK_SIZE,
                        descriptors: Some(Descriptor::Image(
                            &img_view,
                            Layout::ShaderReadOnlyOptimal,
                        )),
                    },
                    DescriptorSetWrite {
                        set: descriptor_set.raw(),
                        binding: 1,
                        array_offset: tex_idx % BLOCK_SIZE,
                        descriptors: Some(Descriptor::Sampler(&sampler)),
                    },
                ]);
            }

            imgs.push(LoadedImage {
                mem: ManuallyDrop::new(img_mem),
                img: ManuallyDrop::new(img),
                img_view: ManuallyDrop::new(img_view),
                sampler: ManuallyDrop::new(sampler),
                row_size,
                height: img_data.height(),
                width: img_data.width(),
            });

            staging_bufs.push(staging_buffer);
        }

        // Add start pipeline barriers
        for li in imgs.iter() {
            use hal::image::Access;

            buf.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                Dependencies::empty(),
                &[Barrier::Image {
                    states: (Access::empty(), Layout::Undefined)
                        ..(Access::TRANSFER_WRITE, Layout::TransferDstOptimal),
                    target: &*li.img,
                    families: None,
                    range: SubresourceRange {
                        aspects: Aspects::COLOR,
                        levels: 0..1,
                        layers: 0..1,
                    },
                }],
            );
        }

        // Record copy commands
        for (li, sb) in imgs.iter().zip(staging_bufs.iter()) {
            buf.copy_buffer_to_image(
                &*sb.buf,
                &*li.img,
                Layout::TransferDstOptimal,
                &[BufferImageCopy {
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
                }],
            );
        }
        for li in imgs.iter() {
            use hal::image::Access;

            buf.pipeline_barrier(
                PipelineStage::TOP_OF_PIPE..PipelineStage::TRANSFER,
                Dependencies::empty(),
                &[Barrier::Image {
                    states: (Access::TRANSFER_WRITE, Layout::TransferDstOptimal)
                        ..(Access::SHADER_READ, Layout::ShaderReadOnlyOptimal),
                    target: &*li.img,
                    families: None,
                    range: Self::RESOURCES,
                }],
            );
        }

        buf.finish();

        // Submit command buffer
        self.gpu.queue_groups[self.cmd_queue_idx].queues[0].submit::<_, _, Semaphore, _, _>(
            Submission {
                command_buffers: &[&buf],
                signal_semaphores: std::iter::empty(),
                wait_semaphores: std::iter::empty(),
            },
            Some(&fence),
        );

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
    device: &mut Device,
    allocator: &mut T,
    format: Format,
    usage: ImgUsage,
    img: &I,
) -> Result<(T::Block, Image)>
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
            ViewCapabilities::empty(),
        )
    }
    .map_err::<HalErrorWrapper, _>(|e| e.into())
    .context("Error creating image")?;

    // Allocate memory
    let (block, _) = unsafe {
        let requirements = device.get_image_requirements(&image_ref);

        allocator.alloc(device, requirements.size, requirements.alignment)
    }
    .map_err::<HalErrorWrapper, _>(|e| e.into())
    .context("Error allocating memory")?;

    unsafe {
        device
            .bind_image_memory(&block.memory(), block.range().start, &mut image_ref)
            .map_err::<HalErrorWrapper, _>(|e| e.into())
            .context("Error binding memory to image")?;
    }

    Ok((block, image_ref))
}
