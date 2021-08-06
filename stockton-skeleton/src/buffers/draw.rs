//! A vertex and index buffer set for drawing

use super::staged::StagedBuffer;
use crate::{
    context::RenderingContext,
    mem::{MappableBlock, MemoryPool},
};

use anyhow::{Context, Result};
use hal::buffer::Usage;
use std::mem::ManuallyDrop;

/// Initial size of vertex buffer. TODO: Way of overriding this
pub const INITIAL_VERT_SIZE: u64 = 3 * 3000;

/// Initial size of index buffer. TODO: Way of overriding this
pub const INITIAL_INDEX_SIZE: u64 = 3000;

/// A vertex and index buffer set for drawing
pub struct DrawBuffers<'a, T: Sized, P: MemoryPool, SP: MemoryPool> {
    pub vertex_buffer: ManuallyDrop<StagedBuffer<'a, T, P, SP>>,
    pub index_buffer: ManuallyDrop<StagedBuffer<'a, (u16, u16, u16), P, SP>>,
}

impl<'a, T, P, SP> DrawBuffers<'a, T, P, SP>
where
    P: MemoryPool,
    SP: MemoryPool,
    SP::Block: MappableBlock,
{
    /// Create a new set of drawbuffers given a render context.
    /// This will allocate memory from `P` and `SP`, and currently has a fixed size (WIP).
    pub fn from_context(context: &mut RenderingContext) -> Result<Self> {
        let vert = StagedBuffer::from_context(context, Usage::VERTEX, INITIAL_VERT_SIZE)
            .context("Error creating vertex buffer")?;
        let index = StagedBuffer::from_context(context, Usage::INDEX, INITIAL_INDEX_SIZE)
            .context("Error creating index buffer")?;

        Ok(DrawBuffers {
            vertex_buffer: ManuallyDrop::new(vert),
            index_buffer: ManuallyDrop::new(index),
        })
    }

    /// Destroy all Vulkan objects. Should be called before dropping.
    pub fn deactivate(self, context: &mut RenderingContext) {
        unsafe {
            use core::ptr::read;

            ManuallyDrop::into_inner(read(&self.vertex_buffer)).deactivate(context);
            ManuallyDrop::into_inner(read(&self.index_buffer)).deactivate(context);
        }
    }
}
