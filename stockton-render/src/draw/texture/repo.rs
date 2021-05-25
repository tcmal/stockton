use stockton_levels::prelude::HasTextures;

use super::{
    block::TexturesBlock,
    loader::{BlockRef, LoaderRequest, TextureLoader, TextureLoaderRemains, NUM_SIMULTANEOUS_CMDS},
    resolver::TextureResolver,
    LoadableImage,
};
use crate::types::*;

use std::{
    collections::HashMap,
    marker::PhantomData,
    mem::drop,
    mem::ManuallyDrop,
    sync::{
        mpsc::{channel, Receiver, Sender},
        Arc, RwLock, RwLockReadGuard,
    },
    thread::JoinHandle,
};

use anyhow::{Context, Result};
use hal::{
    prelude::*,
    pso::{DescriptorSetLayoutBinding, DescriptorType, ShaderStageFlags},
    Features,
};
use log::debug;
use thiserror::Error;

/// The number of textures in one 'block'
/// The textures of the loaded file are divided into blocks of this size.
/// Whenever a texture is needed, the whole block its in is loaded.
pub const BLOCK_SIZE: usize = 8;

pub struct TextureRepo<'a> {
    joiner: ManuallyDrop<JoinHandle<Result<TextureLoaderRemains>>>,
    ds_layout: Arc<RwLock<DescriptorSetLayout>>,
    req_send: Sender<LoaderRequest>,
    resp_recv: Receiver<TexturesBlock<DynamicBlock>>,
    blocks: HashMap<BlockRef, Option<TexturesBlock<DynamicBlock>>>,

    _a: PhantomData<&'a ()>,
}

#[derive(Error, Debug)]
pub enum TextureRepoError {
    #[error("No suitable queue family")]
    NoQueueFamilies,

    #[error("Lock poisoned")]
    LockPoisoned,
}

impl<'a> TextureRepo<'a> {
    pub fn new<
        T: 'static + HasTextures + Send + Sync,
        R: 'static + TextureResolver<I> + Send + Sync,
        I: 'static + LoadableImage + Send,
    >(
        device_lock: Arc<RwLock<Device>>,
        adapter: &Adapter,
        texs_lock: Arc<RwLock<T>>,
        resolver: R,
    ) -> Result<Self> {
        let (req_send, req_recv) = channel();
        let (resp_send, resp_recv) = channel();
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                family.queue_type().supports_transfer()
                    && family.max_queues() >= NUM_SIMULTANEOUS_CMDS
            })
            .ok_or(TextureRepoError::NoQueueFamilies)?;

        let gpu = unsafe {
            adapter
                .physical_device
                .open(&[(family, &[1.0])], Features::empty())?
        };

        let device = device_lock
            .write()
            .map_err(|_| TextureRepoError::LockPoisoned)
            .context("Error getting device lock")?;

        let ds_lock = Arc::new(RwLock::new(
            unsafe {
                device.create_descriptor_set_layout(
                    &[
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
                    ],
                    &[],
                )
            }
            .map_err::<HalErrorWrapper, _>(|e| e.into())
            .context("Error creating descriptor set layout")?,
        ));

        drop(device);

        let joiner = {
            let loader = TextureLoader::new(
                device_lock,
                adapter,
                family.id(),
                gpu,
                ds_lock.clone(),
                req_recv,
                resp_send,
                texs_lock,
                resolver,
            )?;

            std::thread::spawn(move || loader.loop_forever())
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

    pub fn get_ds_layout(&self) -> RwLockReadGuard<DescriptorSetLayout> {
        self.ds_layout.read().unwrap()
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

    pub fn attempt_get_descriptor_set(&mut self, block_id: BlockRef) -> Option<&DescriptorSet> {
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

    pub fn deactivate(mut self, device_lock: &mut Arc<RwLock<Device>>) {
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
            for (i, v) in self.blocks.drain() {
                debug!("Deactivating blockref {:?}", i);
                if let Some(block) = v {
                    block.deactivate(
                        &mut device,
                        &mut *remains.tex_allocator,
                        &mut remains.descriptor_allocator,
                    );
                }
            }

            debug!("Deactivated all blocks");

            // Dispose of both allocators
            read(&*remains.tex_allocator).dispose();
            read(&*remains.descriptor_allocator).dispose(&device);

            debug!("Disposed of allocators");
        }
    }
}
