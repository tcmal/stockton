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
    mem::ManuallyDrop,
    pin::Pin,
    sync::mpsc::{channel, Receiver, Sender},
    thread::JoinHandle,
};

use hal::{
    prelude::*,
    pso::{DescriptorSetLayoutBinding, DescriptorType, ShaderStageFlags},
    Features,
};
use log::debug;

/// The number of textures in one 'block'
/// The textures of the loaded file are divided into blocks of this size.
/// Whenever a texture is needed, the whole block its in is loaded.
pub const BLOCK_SIZE: usize = 8;

pub struct TextureRepo<'a> {
    joiner: ManuallyDrop<JoinHandle<Result<TextureLoaderRemains, &'static str>>>,
    ds_layout: Pin<Box<DescriptorSetLayout>>,
    req_send: Sender<LoaderRequest>,
    resp_recv: Receiver<TexturesBlock<DynamicBlock>>,
    blocks: HashMap<BlockRef, Option<TexturesBlock<DynamicBlock>>>,

    _a: PhantomData<&'a ()>,
}

impl<'a> TextureRepo<'a> {
    pub fn new<
        T: HasTextures + Send + Sync,
        R: 'static + TextureResolver<I> + Send + Sync,
        I: 'static + LoadableImage + Send,
    >(
        device: &'static mut Device,
        adapter: &Adapter,
        texs: &'static T,
        resolver: R,
    ) -> Result<Self, &'static str> {
        let (req_send, req_recv) = channel();
        let (resp_send, resp_recv) = channel();
        let family = adapter
            .queue_families
            .iter()
            .find(|family| {
                family.queue_type().supports_transfer()
                    && family.max_queues() >= NUM_SIMULTANEOUS_CMDS
            })
            .unwrap();
        let gpu = unsafe {
            adapter
                .physical_device
                .open(&[(family, &[1.0])], Features::empty())
                .unwrap()
        };

        let mut ds_layout = Box::pin(
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
            .map_err(|_| "Couldn't create descriptor set layout")?,
        );

        let long_ds_pointer: &'static DescriptorSetLayout =
            unsafe { &mut *(&mut *ds_layout as *mut DescriptorSetLayout) };

        let joiner = {
            let loader = TextureLoader::new(
                device,
                adapter,
                family.id(),
                gpu,
                long_ds_pointer,
                req_recv,
                resp_send,
                texs,
                resolver,
            )?;

            std::thread::spawn(move || loader.loop_forever())
        };

        Ok(TextureRepo {
            joiner: ManuallyDrop::new(joiner),
            ds_layout,
            blocks: HashMap::new(),
            req_send,
            resp_recv,
            _a: PhantomData::default(),
        })
    }

    pub fn get_ds_layout(&self) -> &DescriptorSetLayout {
        &*self.ds_layout
    }

    pub fn queue_load(&mut self, block_id: BlockRef) -> Result<(), &'static str> {
        if self.blocks.contains_key(&block_id) {
            return Ok(());
        }

        self.force_queue_load(block_id)
    }

    pub fn force_queue_load(&mut self, block_id: BlockRef) -> Result<(), &'static str> {
        self.req_send
            .send(LoaderRequest::Load(block_id))
            .map_err(|_| "Couldn't send load request")?;

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

    pub fn deactivate(mut self, device: &mut Device) {
        unsafe {
            use std::ptr::read;

            // Join the loader thread
            self.req_send.send(LoaderRequest::End).unwrap();
            let mut remains = read(&*self.joiner).join().unwrap().unwrap();

            // Process any ones that just got done loading
            self.process_responses();

            // Return all the texture memory and descriptors.
            for (i, v) in self.blocks.drain() {
                debug!("Deactivating blockref {:?}", i);
                if let Some(block) = v {
                    block.deactivate(
                        device,
                        &mut *remains.tex_allocator,
                        &mut remains.descriptor_allocator,
                    );
                }
            }

            debug!("Deactivated all blocks");

            // Dispose of both allocators
            read(&*remains.tex_allocator).dispose();
            read(&*remains.descriptor_allocator).dispose(device);

            debug!("Disposed of allocators");
        }
    }
}
