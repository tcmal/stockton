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

//! Error types

use super::draw::target::TargetChainCreationError;

/// An error encountered creating a rendering context.
#[derive(Debug)]
pub enum CreationError {
    TargetChainCreationError(TargetChainCreationError),
    WindowError,
    BadSurface,

    DeviceError(hal::device::CreationError),

    OutOfMemoryError,

    SyncObjectError,

    NoShaderC,
    ShaderCError(shaderc::Error),
    ShaderModuleFailed(hal::device::ShaderError),
    RenderPassError,
    PipelineError(hal::pso::CreationError),
    BufferError(hal::buffer::CreationError),
    BufferNoMemory,

    SwapchainError(hal::window::CreationError),
    ImageViewError(hal::image::ViewError),

    BadDataError,
}

/// An error encountered when rendering.
/// Usually this is out of memory or something happened to the device/surface.
/// You'll likely need to exit or create a new context.
#[derive(Debug, Clone)]
pub enum FrameError {}
