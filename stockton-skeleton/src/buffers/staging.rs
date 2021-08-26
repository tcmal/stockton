//! A buffer that can be written to by the CPU

use crate::{
    context::RenderingContext,
    error::LockPoisoned,
    mem::{Block, MappableBlock, MemoryPool},
    types::*,
};

use std::{mem::ManuallyDrop, ops::Range};

use anyhow::{Context, Result};
use hal::{buffer::Usage, memory::SparseFlags};

/// A buffer that can be written to by the CPU. Usage will be `Usage::TRANSFER_SRC`.
pub struct StagingBuffer<P: MemoryPool> {
    buf: ManuallyDrop<BufferT>,
    mem: ManuallyDrop<P::Block>,
}

impl<P> StagingBuffer<P>
where
    P: MemoryPool,
    P::Block: MappableBlock,
{
    /// Create a new staging buffer from the given RenderingContext. `size` is in bytes.
    pub fn from_context(context: &mut RenderingContext, size: u64) -> Result<Self> {
        context.ensure_memory_pool::<P>()?;

        let mut device = context.lock_device()?;
        let mut mempool = context
            .existing_memory_pool()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        Self::from_device_pool(&mut device, &mut mempool, size)
    }

    /// Create a new staging buffer from the given device and memory pool. `size` is in bytes.
    pub fn from_device_pool(device: &mut DeviceT, mempool: &mut P, size: u64) -> Result<Self> {
        let mut buffer =
            unsafe { device.create_buffer(size, Usage::TRANSFER_SRC, SparseFlags::empty()) }
                .context("Error creating buffer")?;

        let requirements = unsafe { device.get_buffer_requirements(&buffer) };

        let (memory, _) = mempool
            .alloc(device, requirements.size, requirements.alignment)
            .context("Error allocating staging memory")?;

        unsafe { device.bind_buffer_memory(memory.memory(), memory.range().start, &mut buffer) }
            .context("Error binding staging memory to buffer")?;

        Ok(StagingBuffer {
            buf: ManuallyDrop::new(buffer),
            mem: ManuallyDrop::new(memory),
        })
    }

    /// Map the given range to CPU-visible memory, returning a pointer to the start of that range.
    /// inner_range is local to this block of memory, not to the container as a whole.
    pub fn map(&mut self, device: &mut DeviceT, inner_range: Range<u64>) -> Result<*mut u8> {
        <<P as MemoryPool>::Block>::map(&mut *self.mem, device, inner_range)
    }

    /// Remove any mappings present for this staging buffer.
    pub fn unmap(&mut self, device: &mut DeviceT) -> Result<()> {
        self.mem.unmap(device)
    }

    pub fn deactivate_context(self, context: &mut RenderingContext) {
        let mut device = context.lock_device().unwrap();
        let mut mempool = context.existing_memory_pool().unwrap().write().unwrap();

        self.deactivate_device_pool(&mut device, &mut mempool)
    }

    /// Destroy all vulkan objects. This should be called before dropping
    pub fn deactivate_device_pool(self, device: &mut DeviceT, mempool: &mut P) {
        unsafe {
            use std::ptr::read;
            // Destroy buffer
            device.destroy_buffer(read(&*self.buf));
            // Free memory
            mempool.free(device, read(&*self.mem));
        }
    }

    /// Get a reference to the staging buffer's memory.
    pub fn mem(&self) -> &P::Block {
        &self.mem
    }

    /// Get a reference to the staging buffer.
    pub fn buf(&self) -> &ManuallyDrop<BufferT> {
        &self.buf
    }
}
