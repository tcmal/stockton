//! All sorts of buffers

use std::ops::IndexMut;

use crate::{error::EnvironmentError, types::*};

use anyhow::{Context, Result};
use hal::{
    buffer::Usage,
    memory::{Properties, SparseFlags},
    MemoryTypeId,
};

mod dedicated_image;
mod draw_buffers;
mod staged;

pub use dedicated_image::*;
pub use draw_buffers::*;
pub use staged::*;

/// Create a buffer of the given specifications, allocating more device memory.
// TODO: Use a different memory allocator?
pub(crate) fn create_buffer(
    device: &mut DeviceT,
    adapter: &Adapter,
    usage: Usage,
    properties: Properties,
    size: u64,
) -> Result<(BufferT, MemoryT)> {
    let mut buffer = unsafe { device.create_buffer(size, usage, SparseFlags::empty()) }
        .context("Error creating buffer")?;

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
        .ok_or(EnvironmentError::NoMemoryTypes)?;

    let memory = unsafe { device.allocate_memory(memory_type_id, requirements.size) }
        .context("Error allocating memory")?;

    unsafe { device.bind_buffer_memory(&memory, 0, &mut buffer) }
        .context("Error binding memory to buffer")?;

    Ok((buffer, memory))
}

/// A buffer that can be modified by the CPU
pub trait ModifiableBuffer: IndexMut<usize> {
    /// Get a handle to the underlying GPU buffer
    fn get_buffer(&mut self) -> &BufferT;

    /// Record the command(s) required to commit changes to this buffer to the given command buffer.
    fn record_commit_cmds<'a>(&'a mut self, cmd_buffer: &mut CommandBufferT) -> Result<()>;
}
