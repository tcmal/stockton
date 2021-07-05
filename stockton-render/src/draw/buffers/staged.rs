//! A buffer that can be written to by the CPU using staging memory

use super::{create_buffer, ModifiableBuffer};
use crate::{error::EnvironmentError, types::*};

use core::mem::{size_of, ManuallyDrop};
use std::{
    convert::TryInto,
    iter::{empty, once},
    ops::{Index, IndexMut},
};

use anyhow::{Context, Result};
use hal::{
    buffer::Usage,
    memory::{Properties, Segment, SparseFlags},
    MemoryTypeId,
};

/// A GPU buffer that is written to using a staging buffer
pub struct StagedBuffer<'a, T: Sized> {
    /// CPU-visible buffer
    staged_buffer: ManuallyDrop<BufferT>,

    /// CPU-visible memory
    staged_memory: ManuallyDrop<MemoryT>,

    /// GPU Buffer
    buffer: ManuallyDrop<BufferT>,

    /// GPU Memory
    memory: ManuallyDrop<MemoryT>,

    /// Where staged buffer is mapped in CPU memory
    staged_mapped_memory: &'a mut [T],

    /// If staged memory has been changed since last `commit`
    staged_is_dirty: bool,

    /// The highest index in the buffer that's been written to.
    pub highest_used: usize,
}

impl<'a, T: Sized> StagedBuffer<'a, T> {
    /// size is the size in T
    pub fn new(device: &mut DeviceT, adapter: &Adapter, usage: Usage, size: u64) -> Result<Self> {
        // Convert size to bytes
        let size_bytes = size * size_of::<T>() as u64;

        // Get CPU-visible buffer
        let (staged_buffer, mut staged_memory) = create_buffer(
            device,
            adapter,
            Usage::TRANSFER_SRC,
            Properties::CPU_VISIBLE,
            size_bytes,
        )
        .context("Error creating staging buffer")?;

        // Get GPU Buffer
        let (buffer, memory) = create_buffer(
            device,
            adapter,
            Usage::TRANSFER_DST | usage,
            Properties::DEVICE_LOCAL | Properties::COHERENT,
            size_bytes,
        )
        .context("Error creating GPU buffer")?;

        // Map it somewhere and get a slice to that memory
        let staged_mapped_memory = unsafe {
            let ptr = device
                .map_memory(
                    &mut staged_memory,
                    Segment {
                        offset: 0,
                        size: Some(size_bytes),
                    },
                )
                .context("Error mapping staged memory")?;

            std::slice::from_raw_parts_mut(ptr as *mut T, size.try_into()?)
        };

        Ok(StagedBuffer {
            staged_buffer: ManuallyDrop::new(staged_buffer),
            staged_memory: ManuallyDrop::new(staged_memory),
            buffer: ManuallyDrop::new(buffer),
            memory: ManuallyDrop::new(memory),
            staged_mapped_memory,
            staged_is_dirty: false,
            highest_used: 0,
        })
    }

    /// Call this before dropping
    pub(crate) fn deactivate(mut self, device: &mut DeviceT) {
        unsafe {
            device.unmap_memory(&mut self.staged_memory);

            device.free_memory(ManuallyDrop::take(&mut self.staged_memory));
            device.destroy_buffer(ManuallyDrop::take(&mut self.staged_buffer));

            device.free_memory(ManuallyDrop::take(&mut self.memory));
            device.destroy_buffer(ManuallyDrop::take(&mut self.buffer));
        };
    }
}

impl<'a, T: Sized> ModifiableBuffer for StagedBuffer<'a, T> {
    fn get_buffer(&mut self) -> &BufferT {
        &self.buffer
    }

    fn commit<'b>(
        &'b mut self,
        device: &DeviceT,
        command_queue: &mut QueueT,
        command_pool: &mut CommandPoolT,
    ) -> Result<&'b BufferT> {
        // Only commit if there's changes to commit.
        if self.staged_is_dirty {
            // Copy from staged to buffer
            let buf = unsafe {
                use hal::command::{BufferCopy, CommandBufferFlags};
                // Get a command buffer
                let mut buf = command_pool.allocate_one(hal::command::Level::Primary);

                // Put in our copy command
                buf.begin_primary(CommandBufferFlags::ONE_TIME_SUBMIT);
                buf.copy_buffer(
                    &self.staged_buffer,
                    &self.buffer,
                    std::iter::once(BufferCopy {
                        src: 0,
                        dst: 0,
                        size: ((self.highest_used + 1) * size_of::<T>()) as u64,
                    }),
                );
                buf.finish();

                buf
            };

            // Submit it and wait for completion
            // TODO: Proper management of transfer operations
            unsafe {
                let mut copy_finished = device.create_fence(false)?;
                command_queue.submit(
                    once(&buf),
                    empty::<(&SemaphoreT, hal::pso::PipelineStage)>(),
                    empty::<&SemaphoreT>(),
                    Some(&mut copy_finished),
                );

                device
                    .wait_for_fence(&copy_finished, core::u64::MAX)
                    .context("Error waiting for fence")?;

                // Destroy temporary resources
                device.destroy_fence(copy_finished);
                command_pool.free(once(buf));
            }

            self.staged_is_dirty = false;
        }

        Ok(&self.buffer)
    }
}

impl<'a, T: Sized> Index<usize> for StagedBuffer<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.staged_mapped_memory[index]
    }
}

impl<'a, T: Sized> IndexMut<usize> for StagedBuffer<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        self.staged_is_dirty = true;
        if index > self.highest_used {
            self.highest_used = index;
        }
        &mut self.staged_mapped_memory[index]
    }
}
