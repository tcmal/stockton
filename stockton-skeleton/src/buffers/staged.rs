//! A buffer that can be written to by the CPU using staging memory

use crate::{
    context::RenderingContext,
    error::LockPoisoned,
    mem::{Block, MappableBlock, MemoryPool},
    types::*,
};

use core::mem::{size_of, ManuallyDrop};
use std::{
    convert::TryInto,
    ops::{Index, IndexMut},
};

use anyhow::{Context, Result};
use hal::{buffer::Usage, command::BufferCopy, memory::SparseFlags};

/// A GPU buffer that is written to using a staging buffer. The staging buffer and the GPU buffers are the same size,
/// so this isn't optimal in a lot of cases.
pub struct StagedBuffer<'a, T: Sized, P: MemoryPool, SP: MemoryPool> {
    /// CPU-visible buffer
    staged_buffer: ManuallyDrop<BufferT>,

    /// CPU-visible memory
    staged_memory: ManuallyDrop<SP::Block>,

    /// GPU Buffer
    buffer: ManuallyDrop<BufferT>,

    /// GPU Memory
    memory: ManuallyDrop<P::Block>,

    /// Where staged buffer is mapped in CPU memory
    staged_mapped_memory: &'a mut [T],

    /// The highest index in the buffer that's been written to.
    highest_used: usize,
}

impl<'a, T, P, SP> StagedBuffer<'a, T, P, SP>
where
    T: Sized,
    P: MemoryPool,
    SP: MemoryPool,
    SP::Block: MappableBlock,
{
    /// Create an new staged buffer from the given rendering context.
    /// `size` is the size in T. The GPU buffer's usage will be `usage | Usage::TRANSFER_DST` and the staging buffer's usage will be `Usage::TRANSFER_SRC`.
    pub fn from_context(context: &mut RenderingContext, usage: Usage, size: u64) -> Result<Self> {
        // Convert size to bytes
        let size_bytes = size * size_of::<T>() as u64;

        // Make sure our memory pools exist
        context.ensure_memory_pool::<P>()?;
        context.ensure_memory_pool::<SP>()?;

        // Lock the device and memory pools
        let mut device = context.device().write().map_err(|_| LockPoisoned::Device)?;
        let mut mempool = context
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;
        let mut staging_mempool = context
            .existing_memory_pool::<SP>()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        // Staging buffer
        let (staged_buffer, mut staged_memory) = unsafe {
            create_buffer(
                &mut device,
                size_bytes,
                Usage::TRANSFER_SRC,
                &mut *staging_mempool,
            )
            .context("Error creating staging buffer")?
        };

        // GPU Buffer
        let (buffer, memory) = unsafe {
            create_buffer(
                &mut device,
                size_bytes,
                usage | Usage::TRANSFER_DST,
                &mut *mempool,
            )
            .context("Error creating GPU buffer")?
        };

        // Map the staging buffer somewhere
        let staged_mapped_memory = unsafe {
            std::slice::from_raw_parts_mut(
                std::mem::transmute(staged_memory.map(&mut device, 0..size_bytes)?),
                size.try_into()?,
            )
        };

        Ok(StagedBuffer {
            staged_buffer: ManuallyDrop::new(staged_buffer),
            staged_memory: ManuallyDrop::new(staged_memory),
            buffer: ManuallyDrop::new(buffer),
            memory: ManuallyDrop::new(memory),
            staged_mapped_memory,
            highest_used: 0,
        })
    }

    /// Destroy all Vulkan objects. Should be called before dropping.
    pub fn deactivate(mut self, context: &mut RenderingContext) {
        unsafe {
            let device = &mut *context.device().write().unwrap();

            self.staged_memory.unmap(device).unwrap();

            context
                .existing_memory_pool::<SP>()
                .unwrap()
                .write()
                .unwrap()
                .free(device, ManuallyDrop::take(&mut self.staged_memory));

            context
                .existing_memory_pool::<P>()
                .unwrap()
                .write()
                .unwrap()
                .free(device, ManuallyDrop::take(&mut self.memory));

            device.destroy_buffer(ManuallyDrop::take(&mut self.staged_buffer));
            device.destroy_buffer(ManuallyDrop::take(&mut self.buffer));
        };
    }

    /// Get a handle to the underlying GPU buffer
    pub fn get_buffer(&mut self) -> &BufferT {
        &self.buffer
    }

    /// Record the command(s) required to commit changes to this buffer to the given command buffer.
    pub fn record_commit_cmds(&mut self, buf: &mut CommandBufferT) -> Result<()> {
        unsafe {
            buf.copy_buffer(
                &self.staged_buffer,
                &self.buffer,
                std::iter::once(BufferCopy {
                    src: 0,
                    dst: 0,
                    size: ((self.highest_used + 1) * size_of::<T>()) as u64,
                }),
            );
        }

        Ok(())
    }

    /// Get the highest byte in this buffer that's been written to (by the CPU)
    pub fn highest_used(&self) -> usize {
        self.highest_used
    }
}

/// Used internally to create a buffer from a memory pool
unsafe fn create_buffer<P: MemoryPool>(
    device: &mut DeviceT,
    size: u64,
    usage: Usage,
    mempool: &mut P,
) -> Result<(BufferT, P::Block)> {
    let mut buffer = device
        .create_buffer(size, usage, SparseFlags::empty())
        .context("Error creating buffer")?;
    let req = device.get_buffer_requirements(&buffer);

    let (memory, _) = mempool.alloc(device, size, req.alignment)?;

    device
        .bind_buffer_memory(memory.memory(), 0, &mut buffer)
        .context("Error binding memory to buffer")?;

    Ok((buffer, memory))
}

impl<'a, T: Sized, P: MemoryPool, SP: MemoryPool> Index<usize> for StagedBuffer<'a, T, P, SP> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.staged_mapped_memory[index]
    }
}

impl<'a, T: Sized, P: MemoryPool, SP: MemoryPool> IndexMut<usize> for StagedBuffer<'a, T, P, SP> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index > self.highest_used {
            self.highest_used = index;
        }
        &mut self.staged_mapped_memory[index]
    }
}
