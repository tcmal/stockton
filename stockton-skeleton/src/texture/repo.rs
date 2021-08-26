use super::{
    block::TexturesBlock,
    load::TextureLoadConfig,
    loader::{BlockRef, LoaderRequest, TextureLoader, TextureLoaderRemains, NUM_SIMULTANEOUS_CMDS},
    resolver::TextureResolver,
};
use crate::types::*;
use crate::{context::RenderingContext, error::LockPoisoned, mem::MappableBlock};
use crate::{mem::MemoryPool, queue_negotiator::QueueFamilySelector};

use std::{
    array::IntoIter,
    collections::HashMap,
    iter::empty,
    marker::PhantomData,
    mem::ManuallyDrop,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock, RwLockReadGuard,
    },
    thread::JoinHandle,
};

use anyhow::{Context, Result};
use hal::pso::{DescriptorSetLayoutBinding, DescriptorType, ImageDescriptorType, ShaderStageFlags};
use log::debug;

/// The number of textures in one 'block'
/// The textures of the loaded file are divided into blocks of this size.
/// Whenever a texture is needed, the whole block its in is loaded.
pub const BLOCK_SIZE: usize = 8;

/// An easy way to load [`super::LoadableImage`]s into GPU memory using another thread.
/// This assumes each texture has a numeric id, and will group them into blocks of `[BLOCK_SIZE]`,
/// yielding descriptor sets with that many samplers and images.
/// You only need to supply a [`super::resolver::TextureResolver`] and create one from the main thread.
/// Then, use [`get_ds_layout`] in your graphics pipeline.
/// Make sure to call [`process_responses`] every frame.
/// Then, whenever you draw, use [`attempt_get_descriptor_set`] to see if that texture has finished loading,
/// or `queue_load` to start loading it ASAP.

pub struct TextureRepo<TP, SP>
where
    TP: MemoryPool,
    SP: MemoryPool,
    SP::Block: MappableBlock,
{
    joiner: ManuallyDrop<JoinHandle<Result<TextureLoaderRemains>>>,
    ds_layout: Arc<RwLock<DescriptorSetLayoutT>>,
    req_send: Sender<LoaderRequest>,
    resp_recv: Receiver<TexturesBlock<TP>>,
    blocks: HashMap<BlockRef, Option<TexturesBlock<TP>>>,
    _d: PhantomData<(TP, SP)>,
}

impl<TP, SP> TextureRepo<TP, SP>
where
    TP: MemoryPool,
    SP: MemoryPool,
    SP::Block: MappableBlock,
{
    /// Create a new TextureRepo from the given context.
    /// Q should most likely be [`TexLoadQueue`]
    pub fn new<R: 'static + TextureResolver + Send + Sync, Q: QueueFamilySelector>(
        context: &mut RenderingContext,
        config: TextureLoadConfig<R>,
    ) -> Result<Self> {
        // Create Channels
        let (req_send, req_recv) = channel();
        let (resp_send, resp_recv) = channel();
        let device = context.lock_device()?;

        // Create descriptor set layout
        let ds_lock = Arc::new(RwLock::new(
            unsafe {
                device.create_descriptor_set_layout(
                    IntoIter::new([
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
                    empty(),
                )
            }
            .context("Error creating descriptor set layout")?,
        ));

        debug!("Created descriptor set layout {:?}", ds_lock);

        drop(device);

        let joiner = {
            let loader = <TextureLoader<_, TP, SP>>::new::<Q>(
                context,
                ds_lock.clone(),
                (req_recv, resp_send),
                config,
            )?;

            std::thread::spawn(move || loader.loop_until_exit())
        };

        Ok(TextureRepo {
            joiner: ManuallyDrop::new(joiner),
            ds_layout: ds_lock,
            blocks: HashMap::new(),
            req_send,
            resp_recv,
            _d: PhantomData,
        })
    }

    /// Get the descriptor layout used for each texture descriptor
    /// This can be used when creating graphics pipelines.
    pub fn get_ds_layout(&self) -> Result<RwLockReadGuard<DescriptorSetLayoutT>> {
        self.ds_layout
            .read()
            .map_err(|_| LockPoisoned::Other)
            .context("Error locking descriptor set layout")
    }

    /// Ask for the given block to be loaded, if it's not already.
    pub fn queue_load(&mut self, block_id: BlockRef) -> Result<()> {
        if self.blocks.contains_key(&block_id) {
            return Ok(());
        }

        self.force_queue_load(block_id)
    }

    /// Ask for the given block to be loaded, even if it already has been.
    pub fn force_queue_load(&mut self, block_id: BlockRef) -> Result<()> {
        self.req_send
            .send(LoaderRequest::Load(block_id))
            .context("Error queuing texture block load")?;

        self.blocks.insert(block_id, None);

        Ok(())
    }

    /// Get the descriptor set for the given block, if it's loaded.
    pub fn attempt_get_descriptor_set(&mut self, block_id: BlockRef) -> Option<&DescriptorSetT> {
        self.blocks
            .get(&block_id)
            .and_then(|opt| opt.as_ref().map(|z| z.descriptor_set.raw()))
    }

    /// Process any textures that just finished loading. This should be called every frame.
    pub fn process_responses(&mut self) {
        let resp_iter: Vec<_> = self.resp_recv.try_iter().collect();
        for resp in resp_iter {
            debug!("Got block {:?} back from loader", resp.id);
            self.blocks.insert(resp.id, Some(resp));
        }
    }

    /// Destroy all vulkan objects. Should be called before dropping.
    pub fn deactivate(mut self, context: &mut RenderingContext) {
        unsafe {
            use std::ptr::read;

            // Join the loader thread
            self.req_send.send(LoaderRequest::End).unwrap();
            let mut remains = read(&*self.joiner).join().unwrap().unwrap();

            // Process any ones that just got done loading
            self.process_responses();

            let mut tex_allocator = context
                .existing_memory_pool::<TP>()
                .unwrap()
                .write()
                .unwrap();

            // Only now can we lock device without deadlocking
            let mut device = context.lock_device().unwrap();

            // Return all the texture memory and descriptors.
            for (_, v) in self.blocks.drain() {
                if let Some(block) = v {
                    block.deactivate(
                        &mut device,
                        &mut *tex_allocator,
                        &mut remains.descriptor_allocator,
                    );
                }
            }

            // Dispose of the descriptor allocator
            read(&*remains.descriptor_allocator).dispose(&device);

            // Deactivate DS Layout
            let ds_layout = Arc::try_unwrap(self.ds_layout)
                .unwrap()
                .into_inner()
                .unwrap();
            device.destroy_descriptor_set_layout(ds_layout);
        }
    }
}

/// The queue to use when loading textures
pub struct TexLoadQueue;

impl QueueFamilySelector for TexLoadQueue {
    fn is_suitable(&self, family: &QueueFamilyT) -> bool {
        family.queue_type().supports_transfer() && family.max_queues() >= NUM_SIMULTANEOUS_CMDS
    }
}
