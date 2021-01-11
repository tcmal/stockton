/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use core::mem::{size_of, ManuallyDrop};
use std::convert::TryInto;
use std::iter::once;
use std::ops::{Index, IndexMut};

use hal::prelude::*;
use hal::{buffer::Usage, memory::Properties, queue::Submission, MemoryTypeId};

use crate::error::CreationError;
use crate::types::*;

/// Create a buffer of the given specifications, allocating more device memory.
// TODO: Use a different memory allocator?
pub(crate) fn create_buffer(
    device: &mut Device,
    adapter: &Adapter,
    usage: Usage,
    properties: Properties,
    size: u64,
) -> Result<(Buffer, Memory), CreationError> {
    let mut buffer =
        unsafe { device.create_buffer(size, usage) }.map_err(CreationError::BufferError)?;

    let requirements = unsafe { device.get_buffer_requirements(&buffer) };
    let memory_type_id = adapter
        .physical_device
        .memory_properties()
        .memory_types
        .iter()
        .enumerate()
        .find(|&(id, memory_type)| {
            requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(properties)
        })
        .map(|(id, _)| MemoryTypeId(id))
        .ok_or(CreationError::BufferNoMemory)?;

    let memory = unsafe { device.allocate_memory(memory_type_id, requirements.size) }
        .map_err(|_| CreationError::OutOfMemoryError)?;

    unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }
        .map_err(|_| CreationError::BufferNoMemory)?;

    Ok((buffer, memory))
}

/// A buffer that can be modified by the CPU
pub trait ModifiableBuffer: IndexMut<usize> {
    /// Get a handle to the underlying GPU buffer
    fn get_buffer(&mut self) -> &Buffer;

    /// Commit all changes to GPU memory, returning a handle to the GPU buffer
    fn commit<'a>(
        &'a mut self,
        device: &Device,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
    ) -> &'a Buffer;
}

/// A GPU buffer that is written to using a staging buffer
pub struct StagedBuffer<'a, T: Sized> {
    /// CPU-visible buffer
    staged_buffer: ManuallyDrop<Buffer>,

    /// CPU-visible memory
    staged_memory: ManuallyDrop<Memory>,

    /// GPU Buffer
    buffer: ManuallyDrop<Buffer>,

    /// GPU Memory
    memory: ManuallyDrop<Memory>,

    /// Where staged buffer is mapped in CPU memory
    staged_mapped_memory: &'a mut [T],

    /// If staged memory has been changed since last `commit`
    staged_is_dirty: bool,

    /// The highest index in the buffer that's been written to.
    pub highest_used: usize,
}

impl<'a, T: Sized> StagedBuffer<'a, T> {
    /// size is the size in T
    pub fn new(
        device: &mut Device,
        adapter: &Adapter,
        usage: Usage,
        size: u64,
    ) -> Result<Self, CreationError> {
        // Convert size to bytes
        let size_bytes = size * size_of::<T>() as u64;

        // Get CPU-visible buffer
        let (staged_buffer, staged_memory) = create_buffer(
            device,
            adapter,
            Usage::TRANSFER_SRC,
            Properties::CPU_VISIBLE,
            size_bytes,
        )?;

        // Get GPU Buffer
        let (buffer, memory) = create_buffer(
            device,
            adapter,
            Usage::TRANSFER_DST | usage,
            Properties::DEVICE_LOCAL | Properties::COHERENT,
            size_bytes,
        )?;

        // Map it somewhere and get a slice to that memory
        let staged_mapped_memory = unsafe {
            let ptr = device.map_memory(&staged_memory, 0..size_bytes).unwrap(); // TODO

            std::slice::from_raw_parts_mut(ptr as *mut T, size.try_into().unwrap())
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
    pub(crate) fn deactivate(mut self, device: &mut Device) {
        unsafe {
            device.unmap_memory(&self.staged_memory);

            device.free_memory(ManuallyDrop::take(&mut self.staged_memory));
            device.destroy_buffer(ManuallyDrop::take(&mut self.staged_buffer));

            device.free_memory(ManuallyDrop::take(&mut self.memory));
            device.destroy_buffer(ManuallyDrop::take(&mut self.buffer));
        };
    }
}

impl<'a, T: Sized> ModifiableBuffer for StagedBuffer<'a, T> {
    fn get_buffer(&mut self) -> &Buffer {
        &self.buffer
    }

    fn commit<'b>(
        &'b mut self,
        device: &Device,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
    ) -> &'b Buffer {
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
                    &[BufferCopy {
                        src: 0,
                        dst: 0,
                        size: ((self.highest_used + 1) * size_of::<T>()) as u64,
                    }],
                );
                buf.finish();

                buf
            };

            // Submit it and wait for completion
            // TODO: We could use more semaphores or something?
            // TODO: Better error handling
            unsafe {
                let copy_finished = device.create_fence(false).unwrap();
                command_queue.submit::<_, _, Semaphore, _, _>(
                    Submission {
                        command_buffers: &[&buf],
                        wait_semaphores: std::iter::empty::<_>(),
                        signal_semaphores: std::iter::empty::<_>(),
                    },
                    Some(&copy_finished),
                );

                device
                    .wait_for_fence(&copy_finished, core::u64::MAX)
                    .unwrap();

                // Destroy temporary resources
                device.destroy_fence(copy_finished);
                command_pool.free(once(buf));
            }

            self.staged_is_dirty = false;
        }

        &self.buffer
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
