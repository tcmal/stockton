//! Convenience module to reference types that are stored in the backend's enum

pub use hal::prelude::*;

pub type InstanceT = <back::Backend as hal::Backend>::Instance;
pub type DeviceT = <back::Backend as hal::Backend>::Device;
pub type BufferT = <back::Backend as hal::Backend>::Buffer;
pub type MemoryT = <back::Backend as hal::Backend>::Memory;
pub type SurfaceT = <back::Backend as hal::Backend>::Surface;
pub type SemaphoreT = <back::Backend as hal::Backend>::Semaphore;
pub type FenceT = <back::Backend as hal::Backend>::Fence;
pub type CommandPoolT = <back::Backend as hal::Backend>::CommandPool;
pub type CommandBufferT = <back::Backend as hal::Backend>::CommandBuffer;
pub type QueueT = <back::Backend as hal::Backend>::Queue;
pub type QueueFamilyT = <back::Backend as hal::Backend>::QueueFamily;
pub type DescriptorSetLayoutT = <back::Backend as hal::Backend>::DescriptorSetLayout;
pub type DescriptorSetT = <back::Backend as hal::Backend>::DescriptorSet;
pub type PipelineLayoutT = <back::Backend as hal::Backend>::PipelineLayout;
pub type GraphicsPipelineT = <back::Backend as hal::Backend>::GraphicsPipeline;
pub type ShaderModuleT = <back::Backend as hal::Backend>::ShaderModule;
pub type SamplerT = <back::Backend as hal::Backend>::Sampler;
pub type ImageT = <back::Backend as hal::Backend>::Image;
pub type ImageViewT = <back::Backend as hal::Backend>::ImageView;
pub type FramebufferT = <back::Backend as hal::Backend>::Framebuffer;
pub type RenderPassT = <back::Backend as hal::Backend>::RenderPass;

pub type Adapter = hal::adapter::Adapter<back::Backend>;
pub type QueueGroup = hal::queue::QueueGroup<back::Backend>;

pub type DescriptorAllocator = rendy_descriptor::DescriptorAllocator<back::Backend>;
pub type DynamicAllocator = rendy_memory::DynamicAllocator<back::Backend>;
pub type DynamicBlock = rendy_memory::DynamicBlock<back::Backend>;

pub type RDescriptorSet = rendy_descriptor::DescriptorSet<back::Backend>;
