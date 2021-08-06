use super::{loader::BlockRef, repo::BLOCK_SIZE};
use crate::{buffers::image::SampledImage, mem::MemoryPool, types::*};

use arrayvec::ArrayVec;
use std::{iter::once, mem::ManuallyDrop};

/// A block of loaded textures
pub struct TexturesBlock<TP: MemoryPool> {
    pub id: BlockRef,
    pub descriptor_set: ManuallyDrop<RDescriptorSet>,
    pub imgs: ArrayVec<[SampledImage<TP>; BLOCK_SIZE]>,
}

impl<TP: MemoryPool> TexturesBlock<TP> {
    /// Destroy all Vulkan objects. Must be called before dropping.
    pub fn deactivate(
        mut self,
        device: &mut DeviceT,
        tex_alloc: &mut TP,
        desc_alloc: &mut DescriptorAllocator,
    ) {
        unsafe {
            use std::ptr::read;

            // Descriptor set
            desc_alloc.free(once(read(&*self.descriptor_set)));

            // Images
            self.imgs
                .drain(..)
                .map(|x| x.deactivate_with_device_pool(device, tex_alloc))
                .for_each(|_| {});
        }
    }
}
