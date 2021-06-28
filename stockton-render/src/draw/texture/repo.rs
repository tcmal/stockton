use super::{
    block::TexturesBlock,
    load::TextureLoadConfig,
    loader::{BlockRef, LoaderRequest, TextureLoader, TextureLoaderRemains, NUM_SIMULTANEOUS_CMDS},
    resolver::TextureResolver,
};
use crate::draw::queue_negotiator::QueueFamilySelector;
use crate::error::LockPoisoned;
use crate::types::*;

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
use hal::{
    pso::{DescriptorSetLayoutBinding, DescriptorType, ImageDescriptorType, ShaderStageFlags},
    queue::family::QueueFamilyId,
};
use log::debug;

/// The number of textures in one 'block'
/// The textures of the loaded file are divided into blocks of this size.
/// Whenever a texture is needed, the whole block its in is loaded.
pub const BLOCK_SIZE: usize = 8;

pub struct TextureRepo<'a> {
    joiner: ManuallyDrop<JoinHandle<Result<TextureLoaderRemains>>>,
    ds_layout: Arc<RwLock<DescriptorSetLayoutT>>,
    req_send: Sender<LoaderRequest>,
    resp_recv: Receiver<TexturesBlock<DynamicBlock>>,
    blocks: HashMap<BlockRef, Option<TexturesBlock<DynamicBlock>>>,

    _a: PhantomData<&'a ()>,
}

impl<'a> TextureRepo<'a> {
    pub fn new<R: 'static + TextureResolver + Send + Sync>(
        device_lock: Arc<RwLock<DeviceT>>,
        family: QueueFamilyId,
        queue: Arc<RwLock<QueueT>>,
        adapter: &Adapter,
        config: TextureLoadConfig<R>,
    ) -> Result<Self> {
        // Create Channels
        let (req_send, req_recv) = channel();
        let (resp_send, resp_recv) = channel();
        let device = device_lock
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

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
            let loader = TextureLoader::new(
                adapter,
                device_lock.clone(),
                family,
                queue,
                ds_lock.clone(),
                req_recv,
                resp_send,
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
            _a: PhantomData::default(),
        })
    }

    pub fn get_ds_layout(&self) -> Result<RwLockReadGuard<DescriptorSetLayoutT>> {
        self.ds_layout
            .read()
            .map_err(|_| LockPoisoned::Other)
            .context("Error locking descriptor set layout")
    }

    pub fn queue_load(&mut self, block_id: BlockRef) -> Result<()> {
        if self.blocks.contains_key(&block_id) {
            return Ok(());
        }

        self.force_queue_load(block_id)
    }

    pub fn force_queue_load(&mut self, block_id: BlockRef) -> Result<()> {
        self.req_send
            .send(LoaderRequest::Load(block_id))
            .context("Error queuing texture block load")?;

        self.blocks.insert(block_id, None);

        Ok(())
    }

    pub fn attempt_get_descriptor_set(&mut self, block_id: BlockRef) -> Option<&DescriptorSetT> {
        self.blocks
            .get(&block_id)
            .and_then(|opt| opt.as_ref().map(|z| z.descriptor_set.raw()))
    }

    pub fn process_responses(&mut self) {
        let resp_iter: Vec<_> = self.resp_recv.try_iter().collect();
        for resp in resp_iter {
            debug!("Got block {:?} back from loader", resp.id);
            self.blocks.insert(resp.id, Some(resp));
        }
    }

    pub fn deactivate(mut self, device_lock: &mut Arc<RwLock<DeviceT>>) {
        unsafe {
            use std::ptr::read;

            // Join the loader thread
            self.req_send.send(LoaderRequest::End).unwrap();
            let mut remains = read(&*self.joiner).join().unwrap().unwrap();

            // Process any ones that just got done loading
            self.process_responses();

            // Only now can we lock device without deadlocking
            let mut device = device_lock.write().unwrap();

            // Return all the texture memory and descriptors.
            for (_, v) in self.blocks.drain() {
                if let Some(block) = v {
                    block.deactivate(
                        &mut device,
                        &mut *remains.tex_allocator,
                        &mut remains.descriptor_allocator,
                    );
                }
            }

            // Dispose of both allocators
            read(&*remains.tex_allocator).dispose();
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

pub struct TexLoadQueue;

impl QueueFamilySelector for TexLoadQueue {
    fn is_suitable(&self, family: &QueueFamilyT) -> bool {
        family.queue_type().supports_transfer() && family.max_queues() >= NUM_SIMULTANEOUS_CMDS
    }
}
