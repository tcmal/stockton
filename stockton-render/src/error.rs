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
#[derive(Debug, Clone)]
pub enum CreationError {
	
	/// # Hardware
	NoAdapter,
	NoQueueFamily,
	NoPhysicalDevice,

	/// # Sanity
	NoQueueGroup,
	NoCommandQueues,
	NoPresentModes,
	NoCompositeAlphas,
	NoImageFormats,
	NoColor,

	/// # Runtime
	SwapchainFailed (hal::window::CreationError),
	RenderPassFailed (hal::device::OutOfMemory),
	CommandPoolFailed (hal::device::OutOfMemory),
	SemaphoreFailed (hal::device::OutOfMemory),
	FenceFailed (hal::device::OutOfMemory),
	ImageViewFailed (hal::image::ViewError),
	FramebufferFailed (hal::device::OutOfMemory)
}

impl CreationError {
	/// Check if the error is (likely) a hardware error
	pub fn is_hardware(&self) -> bool {
		use self::CreationError::*;
		match &self {
			NoAdapter | NoQueueFamily | NoPhysicalDevice => true,
			_ => false
		}
	}
	/// Check if the error is (possibly) a sanity error.
	pub fn is_sanity(&self) -> bool {
		use self::CreationError::*;
		match &self {
			NoQueueGroup | NoCommandQueues | NoPresentModes |
			NoCompositeAlphas | NoImageFormats | NoColor
				=> true,
			_ => false
		}
	}
	/// Check if the error is (likely) a runtime error.
	pub fn is_runtime(&self) -> bool {
		use self::CreationError::*;
		match &self {
			SwapchainFailed(_) | RenderPassFailed(_) |
			CommandPoolFailed(_) | SemaphoreFailed(_) |
			FenceFailed(_) | ImageViewFailed(_) |
			FramebufferFailed(_) => true,
			_ => false
		}
	}
}

/// An error encountered when rendering.
/// Usually this is out of memory or something happened to the device/surface.
/// You'll likely need to exit or create a new context.
#[derive(Debug, Clone)]
pub enum FrameError {
	/// Error getting the image from the swapchain
	AcquisitionError (hal::window::AcquireError),
	
	/// Error waiting on the frame_presented fence.
	FenceWaitError (hal::device::OomOrDeviceLost),
	
	/// Error resetting the frame_presented fence.
	FenceResetError (hal::device::OutOfMemory),

	/// Error presenting the rendered frame.
	PresentError (hal::window::PresentError)
}