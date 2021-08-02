//! A buffer that can be written to by the CPU using staging memory

use super::{create_buffer, ModifiableBuffer};
use crate::types::*;

use core::mem::{size_of, ManuallyDrop};
use std::{
    convert::TryInto,
    ops::{Index, IndexMut},
};

use anyhow::{Context, Result};
use hal::{
    buffer::Usage,
    command::BufferCopy,
    memory::{Properties, Segment},
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

    fn record_commit_cmds(&mut self, buf: &mut CommandBufferT) -> Result<()> {
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
}

impl<'a, T: Sized> Index<usize> for StagedBuffer<'a, T> {
    type Output = T;

    fn index(&self, index: usize) -> &Self::Output {
        &self.staged_mapped_memory[index]
    }
}

impl<'a, T: Sized> IndexMut<usize> for StagedBuffer<'a, T> {
    fn index_mut(&mut self, index: usize) -> &mut Self::Output {
        if index > self.highest_used {
            self.highest_used = index;
        }
        &mut self.staged_mapped_memory[index]
    }
}
