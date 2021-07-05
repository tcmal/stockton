//! A vertex and index buffer set for drawing

use super::StagedBuffer;
use crate::types::*;

use anyhow::{Context, Result};
use hal::buffer::Usage;
use std::mem::ManuallyDrop;

/// Initial size of vertex buffer. TODO: Way of overriding this
pub const INITIAL_VERT_SIZE: u64 = 3 * 3000;

/// Initial size of index buffer. TODO: Way of overriding this
pub const INITIAL_INDEX_SIZE: u64 = 3000;

/// The buffers used for drawing, ie index and vertex buffer
pub struct DrawBuffers<'a, T: Sized> {
    pub vertex_buffer: ManuallyDrop<StagedBuffer<'a, T>>,
    pub index_buffer: ManuallyDrop<StagedBuffer<'a, (u16, u16, u16)>>,
}

impl<'a, T> DrawBuffers<'a, T> {
    pub fn new(device: &mut DeviceT, adapter: &Adapter) -> Result<DrawBuffers<'a, T>> {
        let vert = StagedBuffer::new(device, adapter, Usage::VERTEX, INITIAL_VERT_SIZE)
            .context("Error creating vertex buffer")?;
        let index = StagedBuffer::new(device, adapter, Usage::INDEX, INITIAL_INDEX_SIZE)
            .context("Error creating index buffer")?;

        Ok(DrawBuffers {
            vertex_buffer: ManuallyDrop::new(vert),
            index_buffer: ManuallyDrop::new(index),
        })
    }

    pub fn deactivate(self, device: &mut DeviceT) {
        unsafe {
            use core::ptr::read;

            ManuallyDrop::into_inner(read(&self.vertex_buffer)).deactivate(device);
            ManuallyDrop::into_inner(read(&self.index_buffer)).deactivate(device);
        }
    }
}
