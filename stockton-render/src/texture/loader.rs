//! Manages the loading/unloading of textures

use super::{
    block::{LoadedImage, TexturesBlock},
    load::{load_image, QueuedLoad, TextureLoadConfig, TextureLoadError, LAYERS, RESOURCES},
    repo::BLOCK_SIZE,
    resolver::TextureResolver,
    PIXEL_SIZE,
};
use crate::{error::LockPoisoned, types::*, utils::find_memory_type_id};

use std::{
    array::IntoIter,
    collections::VecDeque,
    iter::{empty, once},
    mem::{drop, ManuallyDrop},
    sync::{
        mpsc::{Receiver, Sender},
        Arc, RwLock,
    },
    thread::sleep,
    time::Duration,
};

use anyhow::{Context, Result};
use arrayvec::ArrayVec;
use hal::{
    command::{BufferImageCopy, CommandBufferFlags},
    format::{Aspects, Format},
    image::{Access, Extent, Layout, Offset, SubresourceLayers, SubresourceRange},
    memory::{Barrier, Dependencies, Properties as MemProps, SparseFlags},
    pso::{Descriptor, DescriptorSetWrite, ImageDescriptorType, PipelineStage, ShaderStageFlags},
    queue::family::QueueFamilyId,
    MemoryTypeId,
};
use image::{Rgba, RgbaImage};
use log::*;
use rendy_descriptor::{DescriptorRanges, DescriptorSetLayoutBinding, DescriptorType};
use rendy_memory::DynamicConfig;
use thiserror::Error;

/// The number of command buffers to have in flight simultaneously.
pub const NUM_SIMULTANEOUS_CMDS: usize = 2;

/// A reference to a texture of the current map
pub type BlockRef = usize;

/// Manages the loading/unloading of textures
/// This is expected to load the textures, then send the loaded blocks back
pub struct TextureLoader<R: TextureResolver> {
    /// Blocks for which commands have been queued and are done loading once the fence is triggered.
    commands_queued: ArrayVec<[QueuedLoad<DynamicBlock>; NUM_SIMULTANEOUS_CMDS]>,

    /// The command buffers  used and a fence to go with them
    buffers: VecDeque<(FenceT, CommandBufferT)>,

    /// The command pool buffers were allocated from
    pool: ManuallyDrop<CommandPoolT>,

    /// The GPU we're submitting to
    device: Arc<RwLock<DeviceT>>,

    /// The command queue being used
    queue: Arc<RwLock<QueueT>>,

    /// The memory allocator being used for textures
    tex_allocator: ManuallyDrop<DynamicAllocator>,

    /// The memory allocator for staging memory
    staging_allocator: ManuallyDrop<DynamicAllocator>,

    /// Allocator for descriptor sets
    descriptor_allocator: ManuallyDrop<DescriptorAllocator>,

    ds_layout: Arc<RwLock<DescriptorSetLayoutT>>,

    /// Type ID for staging memory
    staging_memory_type: MemoryTypeId,

    /// From adapter, used for determining alignment
    optimal_buffer_copy_pitch_alignment: hal::buffer::Offset,

    /// Configuration for how to find and load textures
    config: TextureLoadConfig<R>,

    /// The channel requests come in.
    /// Requests should reference a texture **block**, for example textures 8..16 is block 1.
    request_channel: Receiver<LoaderRequest>,

    /// The channel blocks are returned to.
    return_channel: Sender<TexturesBlock<DynamicBlock>>,

    /// A filler image for descriptors that aren't needed but still need to be written to
    blank_image: ManuallyDrop<LoadedImage<DynamicBlock>>,
}

#[derive(Error, Debug)]
pub enum TextureLoaderError {
    #[error("Couldn't find a suitable memory type")]
    NoMemoryTypes,
}

impl<R: TextureResolver> TextureLoader<R> {
    pub fn loop_until_exit(mut self) -> Result<TextureLoaderRemains> {
        debug!("TextureLoader starting main loop");
        let mut res = Ok(false);
        while res.is_ok() {
            res = self.main();
            if let Ok(true) = res {
                break;
            }

            sleep(Duration::from_secs(0));
        }

        match res {
            Ok(true) => {
                debug!("Starting to deactivate TextureLoader");

                Ok(self.deactivate())
            }
            Err(r) => Err(r.context("Error in TextureLoader loop")),
            _ => unreachable!(),
        }
    }
    fn main(&mut self) -> Result<bool> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;
        // Check for blocks that are finished, then send them back
        let mut i = 0;
        while i < self.commands_queued.len() {
            let signalled = unsafe { device.get_fence_status(&self.commands_queued[i].fence) }
                .context("Error checking fence status")?;

            if signalled {
                let (assets, mut staging_bufs, block) = self.commands_queued.remove(i).dissolve();
                debug!("Load finished for texture block {:?}", block.id);

                // Destroy staging buffers
                for buf in staging_bufs.drain(..) {
                    buf.deactivate(&mut device, &mut self.staging_allocator);
                }

                self.buffers.push_back(assets);
                self.return_channel
                    .send(block)
                    .context("Error returning texture block")?;
            } else {
                i += 1;
            }
        }

        drop(device);

        // Check for messages to start loading blocks
        let req_iter: Vec<_> = self.request_channel.try_iter().collect();
        for to_load in req_iter {
            match to_load {
                LoaderRequest::Load(to_load) => {
                    // Attempt to load given block
                    debug!("Attempting to queue load for texture block {:?}", to_load);

                    let result = unsafe { self.attempt_queue_load(to_load) };
                    match result {
                        Ok(queued_load) => self.commands_queued.push(queued_load),
                        Err(x) => match x.downcast_ref::<TextureLoadError>() {
                            Some(TextureLoadError::NoResources) => {
                                debug!("No resources, trying again later");
                            }
                            _ => return Err(x).context("Error queuing texture load"),
                        },
                    }
                }
                LoaderRequest::End => return Ok(true),
            }
        }

        Ok(false)
    }

    pub fn new(
        adapter: &Adapter,
        device_lock: Arc<RwLock<DeviceT>>,
        (family, queue_lock): (QueueFamilyId, Arc<RwLock<QueueT>>),
        ds_layout: Arc<RwLock<DescriptorSetLayoutT>>,
        (request_channel, return_channel): (
            Receiver<LoaderRequest>,
            Sender<TexturesBlock<DynamicBlock>>,
        ),
        config: TextureLoadConfig<R>,
    ) -> Result<Self> {
        let mut device = device_lock
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;
        let device_props = adapter.physical_device.properties();

        let type_mask = unsafe {
            use hal::image::{Kind, Tiling, Usage, ViewCapabilities};

            // We create an empty image with the same format as used for textures
            // this is to get the type_mask required, which will stay the same for
            // all colour images of the same tiling. (certain memory flags excluded).

            // Size and alignment don't necessarily stay the same, so we're forced to
            // guess at the alignment for our allocator.

            // TODO: Way to tune these options
            let img = device
                .create_image(
                    Kind::D2(16, 16, 1, 1),
                    1,
                    Format::Rgba8Srgb,
                    Tiling::Optimal,
                    Usage::SAMPLED,
                    SparseFlags::empty(),
                    ViewCapabilities::empty(),
                )
                .context("Error creating test image to get buffer settings")?;

            let type_mask = device.get_image_requirements(&img).type_mask;

            device.destroy_image(img);

            type_mask
        };

        debug!("Using type mask {:?}", type_mask);

        // Tex Allocator
        let mut tex_allocator = {
            let props = MemProps::DEVICE_LOCAL;

            DynamicAllocator::new(
                find_memory_type_id(adapter, type_mask, props)
                    .ok_or(TextureLoaderError::NoMemoryTypes)
                    .context("Couldn't create tex memory allocator")?,
                props,
                DynamicConfig {
                    block_size_granularity: 4 * 32 * 32, // 32x32 image
                    max_chunk_size: u64::pow(2, 63),
                    min_device_allocation: 4 * 32 * 32,
                },
                device_props.limits.non_coherent_atom_size as u64,
            )
        };

        let (staging_memory_type, mut staging_allocator) = {
            let props = MemProps::CPU_VISIBLE | MemProps::COHERENT;
            let t = find_memory_type_id(adapter, u32::MAX, props)
                .ok_or(TextureLoaderError::NoMemoryTypes)
                .context("Couldn't create staging memory allocator")?;
            (
                t,
                DynamicAllocator::new(
                    t,
                    props,
                    DynamicConfig {
                        block_size_granularity: 4 * 32 * 32, // 32x32 image
                        max_chunk_size: u64::pow(2, 63),
                        min_device_allocation: 4 * 32 * 32,
                    },
                    device_props.limits.non_coherent_atom_size as u64,
                ),
            )
        };

        // Pool
        let mut pool = unsafe {
            use hal::pool::CommandPoolCreateFlags;

            device.create_command_pool(family, CommandPoolCreateFlags::RESET_INDIVIDUAL)
        }
        .context("Error creating command pool")?;

        // Command buffers and fences
        debug!("Creating resources...");
        let mut buffers = {
            let mut data = VecDeque::with_capacity(NUM_SIMULTANEOUS_CMDS);

            for _ in 0..NUM_SIMULTANEOUS_CMDS {
                unsafe {
                    data.push_back((
                        device.create_fence(false).context("Error creating fence")?,
                        pool.allocate_one(hal::command::Level::Primary),
                    ));
                };
            }

            data
        };

        let optimal_buffer_copy_pitch_alignment =
            device_props.limits.optimal_buffer_copy_pitch_alignment;

        let blank_image = unsafe {
            Self::get_blank_image(
                &mut device,
                &mut buffers[0].1,
                &queue_lock,
                (&mut staging_allocator, &mut tex_allocator),
                staging_memory_type,
                optimal_buffer_copy_pitch_alignment,
                &config,
            )
        }
        .context("Error creating blank image")?;

        drop(device);

        Ok(TextureLoader {
            commands_queued: ArrayVec::new(),
            buffers,
            pool: ManuallyDrop::new(pool),
            device: device_lock,
            queue: queue_lock,
            ds_layout,

            tex_allocator: ManuallyDrop::new(tex_allocator),
            staging_allocator: ManuallyDrop::new(staging_allocator),
            descriptor_allocator: ManuallyDrop::new(DescriptorAllocator::new()),

            staging_memory_type,
            optimal_buffer_copy_pitch_alignment,

            request_channel,
            return_channel,
            config,
            blank_image: ManuallyDrop::new(blank_image),
        })
    }

    unsafe fn attempt_queue_load(&mut self, block_ref: usize) -> Result<QueuedLoad<DynamicBlock>> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

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
                    &*self
                        .ds_layout
                        .read()
                        .map_err(|_| LockPoisoned::Other)
                        .context("Error reading descriptor set layout")?,
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
            // Resolve texture
            let img_data = self.config.resolver.resolve(tex_idx as u32);
            if img_data.is_none() {
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

            let img_data = img_data.unwrap();

            let array_offset = tex_idx % BLOCK_SIZE;

            let (staging_buffer, img) = load_image(
                &mut device,
                &mut self.staging_allocator,
                &mut self.tex_allocator,
                self.staging_memory_type,
                self.optimal_buffer_copy_pitch_alignment,
                img_data,
                &self.config,
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

    unsafe fn get_blank_image(
        device: &mut DeviceT,
        buf: &mut CommandBufferT,
        queue_lock: &Arc<RwLock<QueueT>>,
        (staging_allocator, tex_allocator): (&mut DynamicAllocator, &mut DynamicAllocator),
        staging_memory_type: MemoryTypeId,
        obcpa: u64,
        config: &TextureLoadConfig<R>,
    ) -> Result<LoadedImage<DynamicBlock>> {
        let img_data = RgbaImage::from_pixel(1, 1, Rgba([255, 0, 255, 255]));

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
            config,
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
                    width,
                    height,
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

    /// Safely destroy all the vulkan stuff in this instance
    /// Note that this returns the memory allocators, from which should be freed any TextureBlocks
    /// All in-progress things are sent to return_channel.
    fn deactivate(mut self) -> TextureLoaderRemains {
        use std::ptr::read;

        let mut device = self.device.write().unwrap();

        unsafe {
            // Wait for any currently queued loads to be done
            while self.commands_queued.len() > 0 {
                let mut i = 0;
                while i < self.commands_queued.len() {
                    let signalled = device
                        .get_fence_status(&self.commands_queued[i].fence)
                        .expect("Device lost by TextureManager");

                    if signalled {
                        // Destroy finished ones
                        let (assets, mut staging_bufs, block) =
                            self.commands_queued.remove(i).dissolve();

                        device.destroy_fence(assets.0);
                        // Command buffer will be freed when we reset the command pool
                        // Destroy staging buffers
                        for buf in staging_bufs.drain(..) {
                            buf.deactivate(&mut device, &mut self.staging_allocator);
                        }

                        self.return_channel
                            .send(block)
                            .expect("Sending through return channel failed");
                    } else {
                        i += 1;
                    }
                }

                sleep(Duration::from_secs(0));
            }

            // Destroy blank image
            read(&*self.blank_image).deactivate(&mut device, &mut *self.tex_allocator);

            // Destroy fences

            self.buffers
                .drain(..)
                .map(|(f, _)| device.destroy_fence(f))
                .for_each(|_| {});

            // Free command pool
            self.pool.reset(true);
            device.destroy_command_pool(read(&*self.pool));

            debug!("Done deactivating TextureLoader");

            TextureLoaderRemains {
                tex_allocator: ManuallyDrop::new(read(&*self.tex_allocator)),
                descriptor_allocator: ManuallyDrop::new(read(&*self.descriptor_allocator)),
            }
        }
    }
}

pub struct TextureLoaderRemains {
    pub tex_allocator: ManuallyDrop<DynamicAllocator>,
    pub descriptor_allocator: ManuallyDrop<DescriptorAllocator>,
}

pub enum LoaderRequest {
    /// Load the given block
    Load(BlockRef),

    /// Stop looping and deactivate
    End,
}
