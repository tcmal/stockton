// Copyright (C) 2019 Oscar Shrimpton

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Error types

/// An error encountered creating a rendering context.
/// Falls into 3 main types:
/// 	- Hardware - No suitable card usually
/// 	- Sanity - Things that probably aren't true, likely indicating a deeper issue.
///				These aren't guaranteed sanity issues, but they are weird issues.
/// 	- Runtime - Things caused by runtime conditions, usually resource constraints.
/// You can use the associated methods to get the group of one, which may be helpful for error reporting, etc.
#[derive(Debug)]
pub enum CreationError {
	WindowError,
	BadSurface,
	
	DeviceError (hal::error::DeviceCreationError),

	OutOfMemoryError,

	SyncObjectError,
	
	NoShaderC,
	ShaderCError (shaderc::Error),
	ShaderModuleFailed (hal::device::ShaderError),
	RenderPassError,
	PipelineError (hal::pso::CreationError),
	BufferError (hal::buffer::CreationError),
	BufferNoMemory,
	
	SwapchainError (hal::window::CreationError),
	ImageViewError (hal::image::ViewError)
}

/// An error encountered when rendering.
/// Usually this is out of memory or something happened to the device/surface.
/// You'll likely need to exit or create a new context.
#[derive(Debug, Clone)]
pub enum FrameError {
	AcquireError (hal::window::AcquireError),
	SyncObjectError,
	PresentError
}