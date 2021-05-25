//! Manages the loading/unloading of textures

use super::{
    block::TexturesBlock,
    load::{QueuedLoad, TextureLoadError},
    resolver::TextureResolver,
    LoadableImage,
};
use crate::{draw::utils::find_memory_type_id, types::*};

use std::{
    collections::VecDeque,
    marker::PhantomData,
    mem::ManuallyDrop,
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
    format::Format, memory::Properties as MemProps, prelude::*, queue::family::QueueFamilyId,
    MemoryTypeId,
};
use log::*;
use rendy_memory::DynamicConfig;
use stockton_levels::prelude::HasTextures;
use thiserror::Error;

/// The number of command buffers to have in flight simultaneously.
pub const NUM_SIMULTANEOUS_CMDS: usize = 2;

/// A reference to a texture of the current map
pub type BlockRef = usize;

/// Manages the loading/unloading of textures
/// This is expected to load the textures, then send the loaded blocks back
pub struct TextureLoader<T, R, I> {
    /// Handle to the device we're using
    pub(crate) device: Arc<RwLock<Device>>,

    /// Blocks for which commands have been queued and are done loading once the fence is triggered.
    pub(crate) commands_queued: ArrayVec<[QueuedLoad<DynamicBlock>; NUM_SIMULTANEOUS_CMDS]>,

    /// The command buffers  used and a fence to go with them
    pub(crate) buffers: VecDeque<(Fence, CommandBuffer)>,

    /// The command pool buffers were allocated from
    pub(crate) pool: ManuallyDrop<CommandPool>,

    /// The GPU we're submitting to
    pub(crate) gpu: ManuallyDrop<Gpu>,

    /// The index of the command queue being used
    pub(crate) cmd_queue_idx: usize,

    /// The memory allocator being used for textures
    pub(crate) tex_allocator: ManuallyDrop<DynamicAllocator>,

    /// The memory allocator for staging memory
    pub(crate) staging_allocator: ManuallyDrop<DynamicAllocator>,

    /// Allocator for descriptor sets
    pub(crate) descriptor_allocator: ManuallyDrop<DescriptorAllocator>,

    pub(crate) ds_layout: Arc<RwLock<DescriptorSetLayout>>,

    /// Type ID for staging memory
    pub(crate) staging_memory_type: MemoryTypeId,

    /// From adapter, used for determining alignment
    pub(crate) optimal_buffer_copy_pitch_alignment: hal::buffer::Offset,

    /// The textures lump to get info from
    pub(crate) textures: Arc<RwLock<T>>,

    /// The resolver which gets image data for a given texture.
    pub(crate) resolver: R,

    /// The channel requests come in.
    /// Requests should reference a texture **block**, for example textures 8..16 is block 1.
    pub(crate) request_channel: Receiver<LoaderRequest>,

    /// The channel blocks are returned to.
    pub(crate) return_channel: Sender<TexturesBlock<DynamicBlock>>,

    pub(crate) _li: PhantomData<I>,
}

#[derive(Error, Debug)]
pub enum TextureLoaderError {
    #[error("Couldn't find a suitable memory type")]
    NoMemoryTypes,
}

impl<T: HasTextures, R: TextureResolver<I>, I: LoadableImage> TextureLoader<T, R, I> {
    pub fn loop_forever(mut self) -> Result<TextureLoaderRemains> {
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
        let mut device = self.device.write().unwrap();

        // Check for blocks that are finished, then send them back
        let mut i = 0;
        while i < self.commands_queued.len() {
            let signalled = unsafe { device.get_fence_status(&self.commands_queued[i].fence) }
                .map_err::<HalErrorWrapper, _>(|e| e.into())
                .context("Error checking fence status")?;

            if signalled {
                let (assets, mut staging_bufs, block) = self.commands_queued.remove(i).dissolve();
                debug!("Done loading texture block {:?}", block.id);

                // Destroy staging buffers
                while staging_bufs.len() > 0 {
                    let buf = staging_bufs.pop().unwrap();
                    buf.deactivate(&mut device, &mut self.staging_allocator);
                }

                self.buffers.push_back(assets);
                self.return_channel.send(block).unwrap();
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
                    let result = unsafe { self.attempt_queue_load(to_load) };
                    match result {
                        Ok(queued_load) => self.commands_queued.push(queued_load),
                        Err(x) => match x.downcast_ref::<TextureLoadError>() {
                            Some(TextureLoadError::NoResources) => {}
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
        device_lock: Arc<RwLock<Device>>,
        adapter: &Adapter,
        family: QueueFamilyId,
        gpu: Gpu,
        ds_layout: Arc<RwLock<DescriptorSetLayout>>,
        request_channel: Receiver<LoaderRequest>,
        return_channel: Sender<TexturesBlock<DynamicBlock>>,
        texs: Arc<RwLock<T>>,
        resolver: R,
    ) -> Result<Self> {
        let device = device_lock
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        // Pool
        let mut pool = unsafe {
            use hal::pool::CommandPoolCreateFlags;

            device.create_command_pool(family, CommandPoolCreateFlags::RESET_INDIVIDUAL)
        }
        .map_err::<HalErrorWrapper, _>(|e| e.into())
        .context("Error creating command pool")?;

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
                    ViewCapabilities::empty(),
                )
                .map_err::<HalErrorWrapper, _>(|e| e.into())
                .context("Error creating test image to get buffer settings")?;

            let type_mask = device.get_image_requirements(&img).type_mask;

            device.destroy_image(img);

            type_mask
        };

        // Tex Allocator
        let tex_allocator = {
            let props = MemProps::DEVICE_LOCAL;

            DynamicAllocator::new(
                find_memory_type_id(&adapter, type_mask, props)
                    .ok_or(TextureLoaderError::NoMemoryTypes)?,
                props,
                DynamicConfig {
                    block_size_granularity: 4 * 32 * 32, // 32x32 image
                    max_chunk_size: u64::pow(2, 63),
                    min_device_allocation: 4 * 32 * 32,
                },
            )
        };

        let (staging_memory_type, staging_allocator) = {
            let props = MemProps::CPU_VISIBLE | MemProps::COHERENT;
            let t = find_memory_type_id(&adapter, type_mask, props)
                .ok_or(TextureLoaderError::NoMemoryTypes)?;
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
                ),
            )
        };

        let buffers = {
            let mut data = VecDeque::with_capacity(NUM_SIMULTANEOUS_CMDS);

            for _ in 0..NUM_SIMULTANEOUS_CMDS {
                unsafe {
                    data.push_back((
                        device
                            .create_fence(false)
                            .map_err::<HalErrorWrapper, _>(|e| e.into())
                            .context("Error creating fence")?,
                        pool.allocate_one(hal::command::Level::Primary),
                    ));
                };
            }

            data
        };

        let cmd_queue_idx = gpu
            .queue_groups
            .iter()
            .position(|x| x.family == family)
            .unwrap();

        std::mem::drop(device);

        Ok(TextureLoader {
            device: device_lock,
            commands_queued: ArrayVec::new(),
            buffers,
            pool: ManuallyDrop::new(pool),
            gpu: ManuallyDrop::new(gpu),
            cmd_queue_idx,
            ds_layout,

            tex_allocator: ManuallyDrop::new(tex_allocator),
            staging_allocator: ManuallyDrop::new(staging_allocator),
            descriptor_allocator: ManuallyDrop::new(DescriptorAllocator::new()),

            staging_memory_type,
            optimal_buffer_copy_pitch_alignment: adapter
                .physical_device
                .limits()
                .optimal_buffer_copy_pitch_alignment,

            request_channel,
            return_channel,
            textures: texs,
            resolver,
            _li: PhantomData::default(),
        })
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
                        while staging_bufs.len() > 0 {
                            let buf = staging_bufs.pop().unwrap();
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

            // Destroy fences
            let vec: Vec<_> = self.buffers.drain(..).collect();

            vec.into_iter()
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
