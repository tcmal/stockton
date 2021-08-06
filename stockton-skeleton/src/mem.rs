//! Used to represent access different memory 'pools'.
//! Ideally, each pool is optimised for a specific use case.
//! You can implement your own pools using whatever algorithm you'd like. You just need to implement [`MemoryPool`] and optionally [`Block`], then access it
//! using [`RenderingContext.pool_allocator`]
//! Alternatively, some default memory pools are availble when the feature `rendy_pools` is used (on by default).

use crate::{context::RenderingContext, types::*};

use std::{
    ops::Range,
    sync::{Arc, RwLock},
};

use anyhow::Result;
use hal::memory::Properties;

/// An allocator whose memory and allocation pattern is optimised for a specific use case.
pub trait MemoryPool: Send + Sync + 'static {
    /// The block returned by this pool
    type Block: Block + Send + Sync;

    /// Create a new memory pool from the given context
    /// This is called to lazily initialise the memory pool when it is first requested.
    /// It can do any sort of filtering on memory types required.
    fn from_context(context: &RenderingContext) -> Result<Arc<RwLock<Self>>>;

    /// Allocate block of memory.
    /// On success returns allocated block and amount of memory consumed from device.
    /// The returned block must not overlap with any other allocated block, the start of it must be `0 mod(align)`,
    /// and it must be at least `size` bytes.
    fn alloc(&mut self, device: &DeviceT, size: u64, align: u64) -> Result<(Self::Block, u64)>;

    /// Free block of memory.
    /// Returns amount of memory returned to the device.
    /// If the given block was not allocated from this pool, this should be a no-op and should return 0.
    fn free(&mut self, device: &DeviceT, block: Self::Block) -> u64;

    /// Deactivate this memory pool, freeing any allocated memory objects.
    fn deactivate(self, context: &mut RenderingContext);
}

/// Block that owns a `Range` of the `Memory`.
/// Provides access to safe memory range mapping.
pub trait Block {
    /// Get memory properties of the block.
    fn properties(&self) -> Properties;

    /// Get raw memory object.
    fn memory(&self) -> &MemoryT;

    /// Get memory range owned by this block.
    fn range(&self) -> Range<u64>;

    /// Get size of the block.
    fn size(&self) -> u64 {
        let range = self.range();
        range.end - range.start
    }
}

/// An additional trait for [`Block`]s that can be mapped to CPU-visible memory.
///
/// This should only be implemented for blocks that are *guaranteed* to be visible to the CPU
/// and may panic if this is not the case.
pub trait MappableBlock: Block {
    /// Attempt to map this block to CPU-visible memory.
    /// `inner_range` is counted from only inside this block, not the wider memory object this block is a part of
    fn map(&mut self, device: &mut DeviceT, inner_range: Range<u64>) -> Result<*mut u8>;

    /// Unmap this block from CPU-visible memory.
    /// If this block is not mapped, this should be a no-op.
    /// Implementors should ensure that this does not accidentally unmap other blocks using the same memory block.
    fn unmap(&mut self, device: &mut DeviceT) -> Result<()>;
}

#[cfg(feature = "rendy-pools")]
mod rendy {
    use super::*;

    use crate::{
        error::{EnvironmentError, LockPoisoned, UsageError},
        utils::find_memory_type_id,
    };

    use anyhow::{anyhow, Context, Result};
    use hal::{
        format::Format,
        memory::{Properties as MemProps, SparseFlags},
    };
    use rendy_memory::{Allocator, Block as RBlock, DynamicAllocator, DynamicBlock, DynamicConfig};

    /// So we can use rendy blocks as our blocks
    impl<T: RBlock<back::Backend>> Block for T {
        fn properties(&self) -> Properties {
            <T as RBlock<back::Backend>>::properties(&self)
        }

        fn memory(&self) -> &MemoryT {
            <T as RBlock<back::Backend>>::memory(&self)
        }

        fn range(&self) -> Range<u64> {
            <T as RBlock<back::Backend>>::range(&self)
        }
    }

    /// Intended to be used for textures.
    /// The allocated memory is guaranteed to be suitable for any colour image with optimal tiling and no extra sparse flags or view capabilities.
    pub struct TexturesPool(DynamicAllocator<back::Backend>);
    impl MemoryPool for TexturesPool {
        type Block = DynamicBlock<back::Backend>;

        fn alloc(&mut self, device: &DeviceT, size: u64, align: u64) -> Result<(Self::Block, u64)> {
            Ok(self.0.alloc(device, size, align)?)
        }

        fn free(&mut self, device: &DeviceT, block: Self::Block) -> u64 {
            self.0.free(device, block)
        }

        fn from_context(context: &RenderingContext) -> Result<Arc<RwLock<Self>>> {
            let type_mask = unsafe {
                use hal::image::{Kind, Tiling, Usage, ViewCapabilities};

                // We create an empty image with the same format as used for textures
                // this is to get the type_mask required, which will stay the same for
                // all colour images of the same tiling. (certain memory flags excluded).

                // Size and alignment don't necessarily stay the same, so we're forced to
                // guess at the alignment for our allocator.
                let device = context.device().write().map_err(|_| LockPoisoned::Device)?;
                let img = device
                    .create_image(
                        Kind::D2(16, 16, 1, 1),
                        1,
                        Format::Rgba8Srgb,
                        Tiling::Optimal,
                        Usage::SAMPLED,
                        SparseFlags::empty(),
                        ViewCapabilities::empty(),
                    )
                    .context("Error creating test image to get buffer settings")?;

                let type_mask = device.get_image_requirements(&img).type_mask;

                device.destroy_image(img);

                type_mask
            };

            let allocator = {
                let props = MemProps::DEVICE_LOCAL;

                DynamicAllocator::new(
                    find_memory_type_id(context.adapter(), type_mask, props)
                        .ok_or(EnvironmentError::NoMemoryTypes)?,
                    props,
                    DynamicConfig {
                        block_size_granularity: 4 * 32 * 32, // 32x32 image
                        max_chunk_size: u64::pow(2, 63),
                        min_device_allocation: 4 * 32 * 32,
                    },
                    context
                        .physical_device_properties()
                        .limits
                        .non_coherent_atom_size as u64,
                )
            };

            Ok(Arc::new(RwLock::new(Self(allocator))))
        }

        fn deactivate(self, _context: &mut RenderingContext) {
            self.0.dispose();
        }
    }

    /// Used for depth buffers.
    /// Memory returned is guaranteed to be suitable for any image using `context.target_chain().properties().depth_format` with optimal tiling, and no sparse flags or view capabilities.
    pub struct DepthBufferPool(DynamicAllocator<back::Backend>);
    impl MemoryPool for DepthBufferPool {
        type Block = DynamicBlock<back::Backend>;

        fn alloc(&mut self, device: &DeviceT, size: u64, align: u64) -> Result<(Self::Block, u64)> {
            Ok(self.0.alloc(device, size, align)?)
        }

        fn free(&mut self, device: &DeviceT, block: Self::Block) -> u64 {
            self.0.free(device, block)
        }

        fn from_context(context: &RenderingContext) -> Result<Arc<RwLock<Self>>> {
            let type_mask = unsafe {
                use hal::image::{Kind, Tiling, Usage, ViewCapabilities};

                let device = context.device().write().map_err(|_| LockPoisoned::Device)?;
                let img = device
                    .create_image(
                        Kind::D2(16, 16, 1, 1),
                        1,
                        context.target_chain().properties().depth_format,
                        Tiling::Optimal,
                        Usage::SAMPLED,
                        SparseFlags::empty(),
                        ViewCapabilities::empty(),
                    )
                    .context("Error creating test image to get buffer settings")?;

                let type_mask = device.get_image_requirements(&img).type_mask;

                device.destroy_image(img);

                type_mask
            };

            let allocator = {
                let props = MemProps::DEVICE_LOCAL;

                DynamicAllocator::new(
                    find_memory_type_id(context.adapter(), type_mask, props)
                        .ok_or(EnvironmentError::NoMemoryTypes)?,
                    props,
                    DynamicConfig {
                        block_size_granularity: 4 * 32 * 32, // 32x32 image
                        max_chunk_size: u64::pow(2, 63),
                        min_device_allocation: 4 * 32 * 32,
                    },
                    context
                        .physical_device_properties()
                        .limits
                        .non_coherent_atom_size as u64,
                )
            };

            Ok(Arc::new(RwLock::new(Self(allocator))))
        }

        fn deactivate(self, _context: &mut RenderingContext) {
            self.0.dispose()
        }
    }

    /// Used for staging buffers
    pub struct StagingPool(DynamicAllocator<back::Backend>);
    impl MemoryPool for StagingPool {
        type Block = MappableRBlock<DynamicBlock<back::Backend>>;

        fn alloc(&mut self, device: &DeviceT, size: u64, align: u64) -> Result<(Self::Block, u64)> {
            let (b, size) = self.0.alloc(device, size, align)?;
            Ok((MappableRBlock::new_unchecked(b), size))
        }

        fn free(&mut self, device: &DeviceT, block: Self::Block) -> u64 {
            self.0.free(device, block.0)
        }

        fn from_context(context: &RenderingContext) -> Result<Arc<RwLock<Self>>> {
            let allocator = {
                let props = MemProps::CPU_VISIBLE | MemProps::COHERENT;
                let t = find_memory_type_id(context.adapter(), u32::MAX, props)
                    .ok_or(EnvironmentError::NoMemoryTypes)?;
                DynamicAllocator::new(
                    t,
                    props,
                    DynamicConfig {
                        block_size_granularity: 4 * 32 * 32, // 32x32 image
                        max_chunk_size: u64::pow(2, 63),
                        min_device_allocation: 4 * 32 * 32,
                    },
                    context
                        .physical_device_properties()
                        .limits
                        .non_coherent_atom_size as u64,
                )
            };

            Ok(Arc::new(RwLock::new(StagingPool(allocator))))
        }

        fn deactivate(self, _context: &mut RenderingContext) {
            self.0.dispose()
        }
    }

    /// Suitable for input data, such as vertices and indices.
    pub struct DataPool(DynamicAllocator<back::Backend>);
    impl MemoryPool for DataPool {
        type Block = DynamicBlock<back::Backend>;

        fn alloc(&mut self, device: &DeviceT, size: u64, align: u64) -> Result<(Self::Block, u64)> {
            Ok(self.0.alloc(device, size, align)?)
        }

        fn free(&mut self, device: &DeviceT, block: Self::Block) -> u64 {
            self.0.free(device, block)
        }

        fn from_context(context: &RenderingContext) -> Result<Arc<RwLock<Self>>> {
            let allocator = {
                let props = MemProps::CPU_VISIBLE | MemProps::COHERENT;
                let t = find_memory_type_id(context.adapter(), u32::MAX, props)
                    .ok_or(EnvironmentError::NoMemoryTypes)?;
                DynamicAllocator::new(
                    t,
                    props,
                    DynamicConfig {
                        block_size_granularity: 4 * 4 * 128, // 128 f32 XYZ[?] vertices
                        max_chunk_size: u64::pow(2, 63),
                        min_device_allocation: 4 * 4 * 128,
                    },
                    context
                        .physical_device_properties()
                        .limits
                        .non_coherent_atom_size as u64,
                )
            };

            Ok(Arc::new(RwLock::new(DataPool(allocator))))
        }

        fn deactivate(self, _context: &mut RenderingContext) {
            self.0.dispose()
        }
    }

    /// A rendy memory block that is guaranteed to be CPU visible.
    pub struct MappableRBlock<B: RBlock<back::Backend>>(B);
    impl<B: RBlock<back::Backend>> MappableRBlock<B> {
        /// Create a new mappable memory block, returning an error if the block is not CPU visible
        pub fn new(block: B) -> Result<Self> {
            if !block.properties().contains(MemProps::CPU_VISIBLE) {
                return Err(anyhow!(UsageError::NonMappableMemory));
            }
            Ok(Self::new_unchecked(block))
        }

        /// Create a new mappable memory block, without checking if the block is CPU visible.
        pub fn new_unchecked(block: B) -> Self {
            Self(block)
        }
    }

    impl<B: RBlock<back::Backend>> Block for MappableRBlock<B> {
        fn properties(&self) -> MemProps {
            self.0.properties()
        }

        fn memory(&self) -> &MemoryT {
            self.0.memory()
        }

        fn range(&self) -> Range<u64> {
            self.0.range()
        }
    }
    impl<B: RBlock<back::Backend>> MappableBlock for MappableRBlock<B> {
        fn map(&mut self, device: &mut DeviceT, inner_range: Range<u64>) -> Result<*mut u8> {
            unsafe { Ok(self.0.map(device, inner_range)?.ptr().as_mut()) }
        }

        fn unmap(&mut self, device: &mut DeviceT) -> Result<()> {
            Ok(self.0.unmap(device))
        }
    }
}

#[cfg(feature = "rendy-pools")]
pub use rendy::*;
