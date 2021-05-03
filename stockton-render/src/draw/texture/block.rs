use super::{loader::BlockRef, repo::BLOCK_SIZE};
use crate::types::*;

use arrayvec::ArrayVec;
use hal::prelude::*;
use rendy_memory::{Allocator, Block};
use std::{iter::once, mem::ManuallyDrop};

pub struct TexturesBlock<B: Block<back::Backend>> {
    pub id: BlockRef,
    pub descriptor_set: ManuallyDrop<RDescriptorSet>,
    pub imgs: ArrayVec<[LoadedImage<B>; BLOCK_SIZE]>,
}

impl<B: Block<back::Backend>> TexturesBlock<B> {
    pub fn deactivate<T: Allocator<back::Backend, Block = B>>(
        mut self,
        device: &mut Device,
        tex_alloc: &mut T,
        desc_alloc: &mut DescriptorAllocator,
    ) {
        unsafe {
            use std::ptr::read;

            // Descriptor set
            desc_alloc.free(once(read(&*self.descriptor_set)));

            // Images
            self.imgs
                .drain(..)
                .map(|x| x.deactivate(device, tex_alloc))
                .for_each(|_| {});
        }
    }
}

pub struct LoadedImage<B: Block<back::Backend>> {
    pub mem: ManuallyDrop<B>,
    pub img: ManuallyDrop<Image>,
    pub img_view: ManuallyDrop<ImageView>,
    pub sampler: ManuallyDrop<Sampler>,
    pub row_size: usize,
    pub height: u32,
    pub width: u32,
}

impl<B: Block<back::Backend>> LoadedImage<B> {
    pub fn deactivate<T: Allocator<back::Backend, Block = B>>(
        self,
        device: &mut Device,
        alloc: &mut T,
    ) {
        unsafe {
            use std::ptr::read;

            device.destroy_image_view(read(&*self.img_view));
            device.destroy_image(read(&*self.img));
            device.destroy_sampler(read(&*self.sampler));

            alloc.free(device, read(&*self.mem));
        }
    }
}
