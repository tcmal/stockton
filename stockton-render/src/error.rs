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
