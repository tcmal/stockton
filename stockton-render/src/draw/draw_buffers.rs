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

use crate::{
    draw::{buffer::StagedBuffer, UVPoint},
    error::CreationError,
    types::*,
};
use hal::buffer::Usage;
use std::mem::ManuallyDrop;

/// Initial size of vertex buffer. TODO: Way of overriding this
pub const INITIAL_VERT_SIZE: u64 = 3 * 3000;

/// Initial size of index buffer. TODO: Way of overriding this
pub const INITIAL_INDEX_SIZE: u64 = 3000;

/// The buffers used for drawing, ie index and vertex buffer
pub struct DrawBuffers<'a> {
    pub vertex_buffer: ManuallyDrop<StagedBuffer<'a, UVPoint>>,
    pub index_buffer: ManuallyDrop<StagedBuffer<'a, (u16, u16, u16)>>,
}

impl<'a> DrawBuffers<'a> {
    pub fn new(device: &mut Device, adapter: &Adapter) -> Result<DrawBuffers<'a>, CreationError> {
        let vert = StagedBuffer::new(device, &adapter, Usage::VERTEX, INITIAL_VERT_SIZE)?;
        let index = StagedBuffer::new(device, &adapter, Usage::INDEX, INITIAL_INDEX_SIZE)?;

        Ok(DrawBuffers {
            vertex_buffer: ManuallyDrop::new(vert),
            index_buffer: ManuallyDrop::new(index),
        })
    }

    pub fn deactivate(self, device: &mut Device) {
        unsafe {
            use core::ptr::read;

            ManuallyDrop::into_inner(read(&self.vertex_buffer)).deactivate(device);
            ManuallyDrop::into_inner(read(&self.index_buffer)).deactivate(device);
        }
    }
}
