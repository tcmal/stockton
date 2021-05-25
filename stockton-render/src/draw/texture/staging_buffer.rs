use crate::types::*;

use std::mem::ManuallyDrop;

use anyhow::{Context, Result};
use hal::{device::MapError, prelude::*, MemoryTypeId};
use rendy_memory::{Allocator, Block};

pub struct StagingBuffer {
    pub buf: ManuallyDrop<Buffer>,
    pub mem: ManuallyDrop<DynamicBlock>,
}

impl StagingBuffer {
    const USAGE: hal::buffer::Usage = hal::buffer::Usage::TRANSFER_SRC;

    pub fn new(
        device: &mut Device,
        alloc: &mut DynamicAllocator,
        size: u64,
        _memory_type_id: MemoryTypeId,
    ) -> Result<StagingBuffer> {
        let mut buffer = unsafe { device.create_buffer(size, Self::USAGE) }
            .map_err::<HalErrorWrapper, _>(|e| e.into())
            .context("Error creating buffer")?;

        let requirements = unsafe { device.get_buffer_requirements(&buffer) };

        let (memory, _) = alloc
            .alloc(device, requirements.size, requirements.alignment)
            .map_err::<HalErrorWrapper, _>(|e| e.into())
            .context("Error allocating staging memory")?;

        unsafe { device.bind_buffer_memory(memory.memory(), 0, &mut buffer) }
            .map_err::<HalErrorWrapper, _>(|e| e.into())
            .context("Error binding staging memory to buffer")?;

        Ok(StagingBuffer {
            buf: ManuallyDrop::new(buffer),
            mem: ManuallyDrop::new(memory),
        })
    }

    pub unsafe fn map_memory(&mut self, device: &mut Device) -> Result<*mut u8, MapError> {
        device.map_memory(self.mem.memory(), self.mem.range())
    }
    pub unsafe fn unmap_memory(&mut self, device: &mut Device) {
        device.unmap_memory(self.mem.memory()); // TODO: What if the same Memory is mapped in multiple places?
    }

    pub fn deactivate(self, device: &mut Device, alloc: &mut DynamicAllocator) {
        unsafe {
            use std::ptr::read;
            // Destroy buffer
            device.destroy_buffer(read(&*self.buf));
            // Free memory
            alloc.free(device, read(&*self.mem));
        }
    }
}
