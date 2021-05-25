//! Convenience module to reference types that are stored in the backend's enum

use thiserror::Error;

pub type Device = <back::Backend as hal::Backend>::Device;
pub type Gpu = hal::adapter::Gpu<back::Backend>;
pub type Buffer = <back::Backend as hal::Backend>::Buffer;
pub type Memory = <back::Backend as hal::Backend>::Memory;
pub type Swapchain = <back::Backend as hal::Backend>::Swapchain;
pub type Surface = <back::Backend as hal::Backend>::Surface;
pub type Semaphore = <back::Backend as hal::Backend>::Semaphore;
pub type Fence = <back::Backend as hal::Backend>::Fence;
pub type CommandPool = <back::Backend as hal::Backend>::CommandPool;
pub type CommandBuffer = <back::Backend as hal::Backend>::CommandBuffer;
pub type CommandQueue = <back::Backend as hal::Backend>::CommandQueue;
pub type DescriptorSetLayout = <back::Backend as hal::Backend>::DescriptorSetLayout;
pub type DescriptorSet = <back::Backend as hal::Backend>::DescriptorSet;
pub type PipelineLayout = <back::Backend as hal::Backend>::PipelineLayout;
pub type GraphicsPipeline = <back::Backend as hal::Backend>::GraphicsPipeline;
pub type ShaderModule = <back::Backend as hal::Backend>::ShaderModule;
pub type Sampler = <back::Backend as hal::Backend>::Sampler;
pub type Image = <back::Backend as hal::Backend>::Image;
pub type ImageView = <back::Backend as hal::Backend>::ImageView;
pub type Framebuffer = <back::Backend as hal::Backend>::Framebuffer;
pub type RenderPass = <back::Backend as hal::Backend>::RenderPass;

pub type Adapter = hal::adapter::Adapter<back::Backend>;
pub type QueueGroup = hal::queue::QueueGroup<back::Backend>;

pub type DescriptorAllocator = rendy_descriptor::DescriptorAllocator<back::Backend>;
pub type DynamicAllocator = rendy_memory::DynamicAllocator<back::Backend>;
pub type DynamicBlock = rendy_memory::DynamicBlock<back::Backend>;

pub type RDescriptorSet = rendy_descriptor::DescriptorSet<back::Backend>;

#[derive(Error, Debug)]
pub enum LockPoisoned {
    #[error("Device lock poisoned")]
    Device,

    #[error("Map lock poisoned")]
    Map,

    #[error("Other lock poisoned")]
    Other,
}

#[derive(Error, Debug)]
pub enum HalErrorWrapper {
    #[error("Device Creation Error: {0}")]
    DeviceCreationError(#[from] hal::device::CreationError),

    #[error("Buffer Creation Error: {0}")]
    BufferCreationError(#[from] hal::buffer::CreationError),

    #[error("Image Creation Error: {0}")]
    ImageCreationError(#[from] hal::image::CreationError),

    #[error("View Error: {0}")]
    ImageViewError(#[from] hal::image::ViewError),

    #[error("Out of memory on {0}")]
    OutOfMemory(#[from] hal::device::OutOfMemory),

    #[error("Device Lost: {0}")]
    DeviceLost(#[from] hal::device::DeviceLost),

    #[error("Allocation Error: {0}")]
    AllocationError(#[from] hal::device::AllocationError),

    #[error("Bind Error: {0}")]
    BindError(#[from] hal::device::BindError),

    #[error("Map Error: {0}")]
    MapError(#[from] hal::device::MapError),
}
