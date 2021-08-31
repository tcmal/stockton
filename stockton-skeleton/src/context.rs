//! Deals with all the Vulkan/HAL details.
//! This relies on draw passes for the actual drawing logic.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    marker::PhantomData,
    mem::ManuallyDrop,
    ptr::read,
    sync::{Arc, RwLock, RwLockWriteGuard},
};

use anyhow::{anyhow, Context, Result};
use hal::{
    format::{ChannelType, Format, ImageFeature},
    image::{Extent, FramebufferAttachment, Usage, ViewCapabilities},
    pool::CommandPoolCreateFlags,
    pso::Viewport,
    queue::QueueFamilyId,
    window::{CompositeAlphaMode, PresentMode},
    PhysicalDeviceProperties,
};
use log::debug;

use winit::window::Window;

use super::{
    draw_passes::{DrawPass, IntoDrawPass},
    queue_negotiator::{DrawQueue, QueueNegotiator},
    target::TargetChain,
};
use crate::{
    draw_passes::Singular,
    error::{EnvironmentError, LockPoisoned, UsageError},
    mem::MemoryPool,
    queue_negotiator::{QueueFamilyNegotiator, QueueFamilySelector, SharedQueue},
    types::*,
    session::Session
};

/// The actual data behind [`StatefulRenderingContext`]
struct InnerRenderingContext {
    /// Vulkan Instance
    instance: ManuallyDrop<back::Instance>,

    /// Device we're using
    device: Arc<RwLock<DeviceT>>,

    /// Adapter we're using
    adapter: Adapter,

    /// The properties of the physical device we're using
    physical_device_properties: PhysicalDeviceProperties,

    /// Swapchain and stuff
    target_chain: ManuallyDrop<TargetChain>,

    // Command pool and buffers
    /// The command pool used for our buffers
    cmd_pool: ManuallyDrop<CommandPoolT>,

    /// The queue negotiator to use
    queue_negotiator: QueueNegotiator,

    /// The queue to use for drawing
    queue: Arc<RwLock<QueueT>>,

    ///  Number of pixels per standard point
    pixels_per_point: f32,

    /// The list of memory pools
    memory_pools: HashMap<TypeId, Box<dyn Any>>,

    /// Shared properties for this context
    properties: ContextProperties,
}

/// A type enum for different states the `RenderingContext` can be in.
pub trait RenderingContextState: private::Sealed {}

/// Normal operation.
pub struct Normal;
impl RenderingContextState for Normal {}

/// The last draw failed, most likely meaning the surface needs re-created, or the entire context is toast.
pub struct LastDrawFailed;
impl RenderingContextState for LastDrawFailed {}

/// All memory pools have been deactivated. This should only be used when shutting down
pub struct DeactivatedMemoryPools;
impl RenderingContextState for DeactivatedMemoryPools {}

/// Seal `RenderingContextState`
mod private {
    pub trait Sealed {}
    impl Sealed for super::Normal {}
    impl Sealed for super::LastDrawFailed {}
    impl Sealed for super::DeactivatedMemoryPools {}
}

/// Contains most root vulkan objects, and some precalculated info such as best formats to use.
/// In most cases, this and the DrawPass should contain all Vulkan objects present.
/// [`RenderingContext`] is a convenience type that applies in most situations.
pub struct StatefulRenderingContext<S: RenderingContextState>(
    /// The actual data. This is boxed so that there's less overhead when transitioning type state.
    Box<InnerRenderingContext>,
    PhantomData<S>,
);

/// Convenience type, since we often want to refer to normal operation
pub type RenderingContext = StatefulRenderingContext<Normal>;

/// Methods only implemented in normal operation
impl StatefulRenderingContext<Normal> {
    /// Create a new RenderingContext for the given window.
    pub fn new<IDP: IntoDrawPass<DP, Singular>, DP: DrawPass<Singular>>(
        window: &Window,
    ) -> Result<Self> {
        // Create surface
        let (instance, surface, mut adapters) = unsafe {
            let instance =
                back::Instance::create("stockton", 1).context("Error creating vulkan instance")?;
            let surface = instance
                .create_surface(window)
                .context("Error creating surface")?;
            let adapters = instance.enumerate_adapters();

            (instance, surface, adapters)
        };

        // TODO: Properly figure out which adapter to use
        let adapter = adapters.remove(0);

        // Queue Negotiator
        let (family_negotiator, surface) = {
            let dq: DrawQueue = DrawQueue { surface };

            let mut qn = QueueFamilyNegotiator::new();

            // Draw Queue
            qn.find(&adapter, &dq, 1)
                .context("Couldn't find draw queue family")?;

            // Auxiliary queues for DP
            IDP::find_aux_queues(&adapter, &mut qn)
                .context("Level pass couldn't populate queue family negotiator")?;

            (qn, dq.surface)
        };

        // Device & Queue groups
        let (device_lock, queue_groups) = {
            // TODO: This sucks, but hal is restrictive on how we can pass this specific argument.

            // Deduplicate families & convert to specific type.
            let open_spec = family_negotiator.get_open_spec(&adapter);

            let gpu = unsafe {
                adapter
                    .physical_device
                    .open(&open_spec.as_vec(), hal::Features::empty())
                    .context("Error opening logical device")?
            };

            (Arc::new(RwLock::new(gpu.device)), gpu.queue_groups)
        };

        let mut queue_negotiator = family_negotiator.finish(queue_groups);

        // Context properties
        let properties = ContextProperties::find_best(&adapter, &surface)
            .context("Error getting context properties")?;

        debug!("Detected context properties: {:?}", properties);

        let (cmd_pool, target_chain) = {
            // Lock device
            let mut device = device_lock
                .write()
                .map_err(|_| LockPoisoned::Device)
                .context("Error getting device lock")?;

            // Command pool
            let mut cmd_pool = unsafe {
                device.create_command_pool(
                    queue_negotiator
                        .family::<DrawQueue>()
                        .ok_or(EnvironmentError::NoSuitableFamilies)?,
                    CommandPoolCreateFlags::RESET_INDIVIDUAL,
                )
            }
            .context("Error creating draw command pool")?;

            // Swapchain and associated resources
            let target_chain = TargetChain::new(&mut device, surface, &mut cmd_pool, &properties)
                .context("Error creating target chain")?;

            (cmd_pool, target_chain)
        };

        let queue = queue_negotiator
            .get_queue::<DrawQueue>()
            .context("Error getting draw queue")?;

        Ok(StatefulRenderingContext(
            Box::new(InnerRenderingContext {
                instance: ManuallyDrop::new(instance),

                device: device_lock,
                physical_device_properties: adapter.physical_device.properties(),
                adapter,

                queue_negotiator,
                queue,

                target_chain: ManuallyDrop::new(target_chain),
                cmd_pool: ManuallyDrop::new(cmd_pool),

                pixels_per_point: window.scale_factor() as f32,
                memory_pools: HashMap::new(),
                properties,
            }),
            PhantomData,
        ))
    }

    /// Draw onto the next frame of the swapchain.
    /// This takes ownership so we can transition to `LastDrawFailed` if an error occurs.
    /// If it does, you can try to recover with [`StatefulRenderingContext::attempt_recovery`]
    pub fn draw_next_frame<DP: DrawPass<Singular>>(
        mut self,
        session: &Session,
        dp: &mut DP,
    ) -> Result<RenderingContext, (anyhow::Error, StatefulRenderingContext<LastDrawFailed>)> {
        if let Err(e) = self.attempt_draw_next_frame(session, dp) {
            Err((e, StatefulRenderingContext(self.0, PhantomData)))
        } else {
            Ok(self)
        }
    }

    /// The actual drawing attempt
    fn attempt_draw_next_frame<DP: DrawPass<Singular>>(
        &mut self,
        session: &Session,
        dp: &mut DP,
    ) -> Result<()> {
        // Lock device & queue. We can't use our nice convenience function, because of borrowing issues
        let mut device = self
            .0
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;
        let mut queue = self
            .0
            .queue
            .write()
            .map_err(|_| LockPoisoned::Queue)
            .context("Error getting draw queue lock")?;

        self.0
            .target_chain
            .do_draw_with(&mut device, &mut queue, dp, session)
            .context("Error preparing next target")?;

        Ok(())
    }

    /// Get the specified memory pool, lazily initialising it if it's not yet present
    pub fn memory_pool<P: MemoryPool>(&mut self) -> Result<&Arc<RwLock<P>>> {
        self.ensure_memory_pool::<P>()?;
        Ok(self.existing_memory_pool::<P>().unwrap())
    }

    /// Allocate memory from the given pool.
    /// See [`crate::mem::MemoryPool::alloc`]
    pub fn alloc<P: MemoryPool>(&mut self, size: u64, align: u64) -> Result<P::Block> {
        self.ensure_memory_pool::<P>()?;

        let device = self.lock_device()?;
        let mut pool = self
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        Ok(pool.alloc(&device, size, align)?.0)
    }

    /// Free memory from the given pool.
    /// See [`crate::mem::MemoryPool::free`]
    pub fn free<P: MemoryPool>(&mut self, block: P::Block) -> Result<()> {
        self.ensure_memory_pool::<P>()?;

        let device = self.lock_device()?;
        let mut pool = self
            .existing_memory_pool::<P>()
            .unwrap()
            .write()
            .map_err(|_| LockPoisoned::MemoryPool)?;

        pool.free(&device, block);

        Ok(())
    }

    /// Ensure the specified memory pool is initialised.
    #[allow(clippy::map_entry)] // We can't follow the suggestion because of a borrowing issue
    pub fn ensure_memory_pool<P: MemoryPool>(&mut self) -> Result<()> {
        let tid = TypeId::of::<P>();
        if !self.0.memory_pools.contains_key(&tid) {
            self.0
                .memory_pools
                .insert(tid, Box::new(P::from_context(self)?));
        }
        Ok(())
    }

    /// Get the specified memory pool, returning None if it's not yet present
    /// You should only use this when you're certain it exists, such as when freeing memory
    /// allocated from that pool
    pub fn existing_memory_pool<P: MemoryPool>(&self) -> Option<&Arc<RwLock<P>>> {
        self.0
            .memory_pools
            .get(&TypeId::of::<P>())
            .map(|x| x.downcast_ref().unwrap())
    }

    /// Deactivate all stored memory pools.
    pub fn deactivate_memory_pools(
        self,
    ) -> Result<StatefulRenderingContext<DeactivatedMemoryPools>> {
        // TODO: Properly deactivate memory pools

        Ok(StatefulRenderingContext(self.0, PhantomData))
    }
}

impl StatefulRenderingContext<LastDrawFailed> {
    /// If this function fails the whole context is probably dead
    pub fn attempt_recovery(self) -> Result<RenderingContext> {
        let this = self.recreate_surface()?;
        Ok(StatefulRenderingContext(this.0, PhantomData))
    }
}

// Methods implemented for all states
impl<S: RenderingContextState> StatefulRenderingContext<S> {
    /// Get the current pixels per point.
    pub fn pixels_per_point(&self) -> f32 {
        self.0.pixels_per_point
    }

    /// Get a new reference to the lock for the device used by this context.
    /// This can be used when instantiating code that runs in another thread.
    pub fn clone_device_lock(&self) -> Arc<RwLock<DeviceT>> {
        self.0.device.clone()
    }

    /// Lock the device used by this rendering context
    pub fn lock_device(&self) -> Result<RwLockWriteGuard<'_, DeviceT>> {
        Ok(self.0.device.write().map_err(|_| LockPoisoned::Device)?)
    }

    /// Get a reference to the rendering context's adapter.
    pub fn adapter(&self) -> &Adapter {
        &self.0.adapter
    }

    /// Get a shared queue from the family that was selected with T.
    /// You should already have called [`crate::queue_negotiator::QueueFamilyNegotiator::find`], otherwise this will return an error.
    pub fn get_queue<T: QueueFamilySelector>(&mut self) -> Result<SharedQueue> {
        self.0.queue_negotiator.get_queue::<T>()
    }

    /// Get the family that was selected by T.
    /// You should already have called [`crate::queue_negotiator::QueueFamilyNegotiator::find`], otherwise this will return an error.
    pub fn get_queue_family<T: QueueFamilySelector>(&self) -> Result<QueueFamilyId> {
        self.0
            .queue_negotiator
            .family::<T>()
            .ok_or(anyhow!(UsageError::QueueNegotiatorMisuse))
    }

    /// Get a reference to the physical device's properties.
    pub fn physical_device_properties(&self) -> &PhysicalDeviceProperties {
        &self.0.physical_device_properties
    }
    /// Get a reference to the rendering context's properties.
    pub fn properties(&self) -> &ContextProperties {
        &self.0.properties
    }

    /// Recreate the surface, swapchain, and other derived components.
    pub fn recreate_surface(mut self) -> Result<Self> {
        // TODO: Deactivate if this fails
        unsafe {
            let mut device = self
                .0
                .device
                .write()
                .map_err(|_| LockPoisoned::Device)
                .context("Error getting device lock")?;

            device
                .wait_idle()
                .context("Error waiting for device to become idle")?;

            let surface = ManuallyDrop::into_inner(read(&self.0.target_chain))
                .deactivate_with_recyling(&mut device, &mut self.0.cmd_pool);

            self.0.properties = ContextProperties::find_best(&self.0.adapter, &surface)
                .context("Error finding best swapchain properties")?;

            // TODO: This is unsound, if we return an error here `self.0.TargetChain` may be accessed again.
            self.0.target_chain = ManuallyDrop::new(
                TargetChain::new(
                    &mut device,
                    surface,
                    &mut self.0.cmd_pool,
                    &self.0.properties,
                )
                .context("Error creating target chain")?,
            );
        }

        Ok(StatefulRenderingContext(self.0, PhantomData))
    }
}

// Methods only implemented after we start deactivating
impl StatefulRenderingContext<DeactivatedMemoryPools> {
    pub fn deactivate(mut self) -> Result<()> {
        self.lock_device()?.wait_idle()?;

        // TODO: The rest of the deactivation code needs updated.
        unsafe {
            let mut device = self.0.device.write().map_err(|_| LockPoisoned::Device)?;

            let target_chain = ManuallyDrop::take(&mut self.0.target_chain);
            target_chain.deactivate(&mut self.0.instance, &mut device, &mut self.0.cmd_pool);

            device.destroy_command_pool(ManuallyDrop::into_inner(self.0.cmd_pool));
        }

        Ok(())
    }
}

/// Common properties shared by this entire context
#[derive(Debug, Clone)]
pub struct ContextProperties {
    /// Format to be used by colour attachments. Used by swapchain images.
    pub color_format: Format,

    /// Recommended format to be used by depth attachments.
    pub depth_format: Format,

    /// The present mode being used by the context
    pub present_mode: PresentMode,

    /// How the swapchain is handling alpha values in the end image
    pub composite_alpha_mode: CompositeAlphaMode,

    pub viewport: Viewport,
    pub extent: Extent,

    /// The maximum number of frames we queue at once.
    pub image_count: u32,
}

impl ContextProperties {
    /// Find the best properties for the given adapter and surface
    pub fn find_best(
        adapter: &Adapter,
        surface: &SurfaceT,
    ) -> Result<ContextProperties, EnvironmentError> {
        let caps = surface.capabilities(&adapter.physical_device);
        let formats = surface.supported_formats(&adapter.physical_device);

        // Use the first SRGB format our surface prefers
        let color_format = match formats {
            Some(formats) => formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .copied()
                .ok_or(EnvironmentError::ColorFormat),
            None => Ok(Format::Rgba8Srgb),
        }?;

        // Use the most preferable format our adapter prefers.
        let depth_format = *[
            Format::D32SfloatS8Uint,
            Format::D24UnormS8Uint,
            Format::D32Sfloat,
        ]
        .iter()
        .find(|format| {
            format.is_depth()
                && adapter
                    .physical_device
                    .format_properties(Some(**format))
                    .optimal_tiling
                    .contains(ImageFeature::DEPTH_STENCIL_ATTACHMENT)
        })
        .ok_or(EnvironmentError::DepthFormat)?;

        // V-Sync if possible
        let present_mode = [
            PresentMode::MAILBOX,
            PresentMode::FIFO,
            PresentMode::RELAXED,
            PresentMode::IMMEDIATE,
        ]
        .iter()
        .cloned()
        .find(|pm| caps.present_modes.contains(*pm))
        .ok_or(EnvironmentError::PresentMode)?;

        // Prefer opaque
        let composite_alpha_mode = [
            CompositeAlphaMode::OPAQUE,
            CompositeAlphaMode::INHERIT,
            CompositeAlphaMode::PREMULTIPLIED,
            CompositeAlphaMode::POSTMULTIPLIED,
        ]
        .iter()
        .cloned()
        .find(|ca| caps.composite_alpha_modes.contains(*ca))
        .ok_or(EnvironmentError::CompositeAlphaMode)?;

        let extent = caps.extents.end().to_extent(); // Size
        let viewport = Viewport {
            rect: extent.rect(),
            depth: 0.0..1.0,
        };

        Ok(ContextProperties {
            color_format,
            depth_format,
            present_mode,
            composite_alpha_mode,
            extent,
            viewport,
            image_count: if present_mode == PresentMode::MAILBOX {
                ((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(3))
            } else {
                ((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(2))
            },
        })
    }

    /// Get the framebuffer attachment to use for swapchain images
    pub fn swapchain_framebuffer_attachment(&self) -> FramebufferAttachment {
        FramebufferAttachment {
            usage: Usage::COLOR_ATTACHMENT,
            format: self.color_format,
            view_caps: ViewCapabilities::empty(),
        }
    }
}
