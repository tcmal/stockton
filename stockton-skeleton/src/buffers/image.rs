//! An image with memory bound to it and an image view into its entirety.
//! This is useful for most types of images.
//! ```rust
//! # use anyhow::Result;
//! # use crate::{mem::DrawAttachments, context::RenderingContext};
//! fn create_depth_buffer(
//!     context: &mut RenderingContext,
//! ) -> Result<BoundImageView<DrawAttachments>> {
//!     BoundImageView::from_context(
//!         context,
//!         &ImageSpec {
//!             width: 10,
//!             height: 10,
//!             format: Format::D32Sfloat,
//!             usage: Usage::DEPTH_STENCIL_ATTACHMENT,
//!         },
//!     )
//! }
/// ```
use std::mem::ManuallyDrop;

use crate::{
    context::RenderingContext,
    error::LockPoisoned,
    mem::{Block, MemoryPool},
    types::*,
    utils::get_pixel_size,
};
use anyhow::{Context, Result};
use hal::{
    format::{Aspects, Format, Swizzle},
    image::{SamplerDesc, SubresourceRange, Usage, ViewKind},
    memory::SparseFlags,
};

pub const COLOR_RESOURCES: SubresourceRange = SubresourceRange {
    aspects: Aspects::COLOR,
    level_start: 0,
    level_count: Some(1),
    layer_start: 0,
    layer_count: Some(1),
};

pub const DEPTH_RESOURCES: SubresourceRange = SubresourceRange {
    aspects: Aspects::DEPTH,
    level_start: 0,
    level_count: Some(1),
    layer_start: 0,
    layer_count: Some(1),
};

/// An image with memory bound to it and an image view into its entirety
/// Memory is allocated from the memory pool P, see [`crate::mem`]
pub struct BoundImageView<P: MemoryPool> {
    mem: ManuallyDrop<P::Block>,
    img: ManuallyDrop<ImageT>,
    img_view: ManuallyDrop<ImageViewT>,
    unpadded_row_size: u32,
    row_size: u32,
    height: u32,
}

impl<P: MemoryPool> BoundImageView<P> {
    /// Create an uninitialised image using memory from the specified pool
    pub fn from_context(context: &mut RenderingContext, spec: &ImageSpec) -> Result<Self> {
        // Ensure the memory pool exists before we get a reference to it
        context
            .ensure_memory_pool::<P>()
            .context("Error creating memory pool requested for BoundImageView")?;
        let mut allocator = context
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        let mut device = context.lock_device()?;
        let row_alignment_mask = context
            .physical_device_properties()
            .limits
            .optimal_buffer_copy_pitch_alignment as u32
            - 1;
        Self::from_device_allocator(&mut device, &mut allocator, row_alignment_mask, spec)
    }

    /// Create an uninitialised image using memory from the specified pool, but using a much less convenient signature.
    /// Use this when you don't have access to the full context.
    pub fn from_device_allocator(
        device: &mut DeviceT,
        pool: &mut P,
        row_alignment_mask: u32,
        spec: &ImageSpec,
    ) -> Result<Self> {
        // Calculate buffer size & alignment
        let initial_row_size = get_pixel_size(spec.format) * spec.width;
        let row_size = (initial_row_size + row_alignment_mask) & !row_alignment_mask;
        debug_assert!(row_size >= initial_row_size);

        unsafe {
            use hal::image::{Kind, Tiling, ViewCapabilities};

            // Create the image
            let mut img = device
                .create_image(
                    Kind::D2(spec.width, spec.height, 1, 1),
                    1,
                    spec.format,
                    Tiling::Optimal,
                    spec.usage,
                    SparseFlags::empty(),
                    ViewCapabilities::empty(),
                )
                .context("Error creating image")?;

            // Get memory requirements
            let requirements = device.get_image_requirements(&img);

            // Allocate memory
            let (mem, _) = pool
                .alloc(device, requirements.size, requirements.alignment)
                .context("Error allocating memory")?;

            // Bind memory
            device
                .bind_image_memory(mem.memory(), mem.range().start, &mut img)
                .context("Error binding memory to image")?;

            // Create image view
            let img_view = device
                .create_image_view(
                    &img,
                    ViewKind::D2,
                    spec.format,
                    Swizzle::NO,
                    spec.usage,
                    spec.resources.clone(),
                )
                .context("Error creating image view")?;

            Ok(Self {
                mem: ManuallyDrop::new(mem),
                img: ManuallyDrop::new(img),
                img_view: ManuallyDrop::new(img_view),
                row_size,
                height: spec.height,
                unpadded_row_size: spec.width,
            })
        }
    }

    /// Destroy all vulkan objects. Must be called before dropping.
    pub fn deactivate_with_context(self, context: &mut RenderingContext) {
        let mut device = context.lock_device().unwrap();
        let mut pool = context
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .unwrap();

        self.deactivate_with_device_pool(&mut device, &mut pool);
    }

    /// Destroy all vulkan objects. Must be called before dropping.
    pub fn deactivate_with_device_pool(self, device: &mut DeviceT, pool: &mut P) {
        use std::ptr::read;
        unsafe {
            device.destroy_image_view(read(&*self.img_view));
            device.destroy_image(read(&*self.img));
            pool.free(device, read(&*self.mem));
        }
    }

    /// Get a reference to the bound image.
    pub fn img(&self) -> &ImageT {
        &*self.img
    }

    /// Get a reference to the view of the bound image.
    pub fn img_view(&self) -> &ImageViewT {
        &*self.img_view
    }

    /// Get a reference to the memory used by the bound image.
    pub fn mem(&self) -> &<P as MemoryPool>::Block {
        &*self.mem
    }

    /// Get the bound image view's row size.
    pub fn row_size(&self) -> u32 {
        self.row_size
    }

    /// Get the bound image view's height.
    pub fn height(&self) -> u32 {
        self.height
    }

    /// Get the bound image view's unpadded row size.
    pub fn unpadded_row_size(&self) -> u32 {
        self.unpadded_row_size
    }
}

/// A [`self::BoundImageView`] and accompanying sampler.
pub struct SampledImage<P: MemoryPool> {
    bound_image: ManuallyDrop<BoundImageView<P>>,
    sampler: ManuallyDrop<SamplerT>,
}

impl<P: MemoryPool> SampledImage<P> {
    /// Create an uninitialised image using memory from the specified pool
    pub fn from_context(
        context: &mut RenderingContext,
        spec: &ImageSpec,
        sampler_desc: &SamplerDesc,
    ) -> Result<Self> {
        // Ensure the memory pool exists before we get a reference to it
        context
            .ensure_memory_pool::<P>()
            .context("Error creating memory pool requested for BoundImageView")?;
        let mut allocator = context
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        let mut device = context.lock_device()?;
        let row_alignment_mask = context
            .physical_device_properties()
            .limits
            .optimal_buffer_copy_pitch_alignment as u32
            - 1;

        Self::from_device_allocator(
            &mut device,
            &mut allocator,
            row_alignment_mask,
            spec,
            sampler_desc,
        )
    }

    /// Create an uninitialised image and sampler using memory from the specified pool, but using a much less convenient signature.
    /// Use this when you don't have access to the full context.
    pub fn from_device_allocator(
        device: &mut DeviceT,
        pool: &mut P,
        row_alignment_mask: u32,
        spec: &ImageSpec,
        sampler_desc: &SamplerDesc,
    ) -> Result<Self> {
        let sampler = unsafe { device.create_sampler(sampler_desc) }?;

        Ok(SampledImage {
            bound_image: ManuallyDrop::new(BoundImageView::from_device_allocator(
                device,
                pool,
                row_alignment_mask,
                spec,
            )?),
            sampler: ManuallyDrop::new(sampler),
        })
    }

    /// Destroy all vulkan objects. Must be called before dropping.
    pub fn deactivate_with_context(self, context: &mut RenderingContext) {
        let mut device = context.lock_device().unwrap();
        let mut pool = context
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .unwrap();

        self.deactivate_with_device_pool(&mut device, &mut pool);
    }

    /// Destroy all vulkan objects. Must be called before dropping.
    pub fn deactivate_with_device_pool(self, device: &mut DeviceT, pool: &mut P) {
        unsafe {
            use std::ptr::read;
            read(&*self.bound_image).deactivate_with_device_pool(device, pool);
            device.destroy_sampler(read(&*self.sampler));
        }
    }

    /// Get a reference to the bound image object.
    pub fn bound_image(&self) -> &BoundImageView<P> {
        &self.bound_image
    }

    /// Get a reference to the bound image.
    pub fn img(&self) -> &ImageT {
        self.bound_image.img()
    }

    /// Get a reference to the view of the bound image.
    pub fn img_view(&self) -> &ImageViewT {
        self.bound_image.img_view()
    }

    /// Get the bound image view's row size.
    pub fn row_size(&self) -> u32 {
        self.bound_image.row_size()
    }

    /// Get the bound image view's unpadded row size.
    pub fn unpadded_row_size(&self) -> u32 {
        self.bound_image.unpadded_row_size()
    }

    /// Get the bound image view's height.
    pub fn height(&self) -> u32 {
        self.bound_image.height()
    }

    /// Get a reference to the memory used by the bound image.
    pub fn mem(&self) -> &<P as MemoryPool>::Block {
        self.bound_image.mem()
    }

    /// Get a reference to the sampler.
    pub fn sampler(&self) -> &SamplerT {
        &self.sampler
    }
}

/// Information needed to create an image.
#[derive(Debug, Clone)]
pub struct ImageSpec {
    pub width: u32,
    pub height: u32,
    pub format: Format,
    pub usage: Usage,
    pub resources: SubresourceRange,
}
