#![allow(mutable_transmutes)]
use crate::types::*;

use std::mem::ManuallyDrop;

use anyhow::{Context, Result};
use hal::{device::MapError, memory::SparseFlags, MemoryTypeId};
use rendy_memory::{Allocator, Block};

pub struct StagingBuffer {
    pub buf: ManuallyDrop<BufferT>,
    pub mem: ManuallyDrop<DynamicBlock>,
}

impl StagingBuffer {
    const USAGE: hal::buffer::Usage = hal::buffer::Usage::TRANSFER_SRC;

    pub fn new(
        device: &mut DeviceT,
        alloc: &mut DynamicAllocator,
        size: u64,
        _memory_type_id: MemoryTypeId,
    ) -> Result<StagingBuffer> {
        let mut buffer = unsafe { device.create_buffer(size, Self::USAGE, SparseFlags::empty()) }
            .context("Error creating buffer")?;

        let requirements = unsafe { device.get_buffer_requirements(&buffer) };

        let (memory, _) = alloc
            .alloc(device, requirements.size, requirements.alignment)
            .context("Error allocating staging memory")?;

        unsafe { device.bind_buffer_memory(memory.memory(), 0, &mut buffer) }
            .context("Error binding staging memory to buffer")?;

        Ok(StagingBuffer {
            buf: ManuallyDrop::new(buffer),
            mem: ManuallyDrop::new(memory),
        })
    }

    pub unsafe fn map_memory(&mut self, device: &mut DeviceT) -> Result<*mut u8, MapError> {
        let range = 0..(self.mem.range().end - self.mem.range().start);
        Ok(self.mem.map(device, range)?.ptr().as_mut())
    }
    pub unsafe fn unmap_memory(&mut self, device: &mut DeviceT) {
        self.mem.unmap(device);
    }

    pub fn deactivate(self, device: &mut DeviceT, alloc: &mut DynamicAllocator) {
        unsafe {
            use std::ptr::read;
            // Destroy buffer
            device.destroy_buffer(read(&*self.buf));
            // Free memory
            alloc.free(device, read(&*self.mem));
        }
    }
}
