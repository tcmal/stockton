//! Deals with all the Vulkan/HAL details.
//! This relies on draw passes for the actual drawing logic.

use std::{
    any::{Any, TypeId},
    collections::HashMap,
    mem::ManuallyDrop,
    ptr::read,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};
use hal::{
    format::{ChannelType, Format, ImageFeature},
    image::{Extent, FramebufferAttachment, Usage, ViewCapabilities},
    pool::CommandPoolCreateFlags,
    pso::Viewport,
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
    error::{EnvironmentError, LockPoisoned},
    mem::MemoryPool,
    queue_negotiator::QueueFamilyNegotiator,
    types::*,
};

use stockton_types::Session;

/// Contains most root vulkan objects, and some precalculated info such as best formats to use.
/// In most cases, this and the DrawPass should contain all vulkan objects present.
pub struct RenderingContext {
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

impl RenderingContext {
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
        let (queue_negotiator, surface) = {
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
            let open_spec = queue_negotiator.get_open_spec(&adapter);

            let gpu = unsafe {
                adapter
                    .physical_device
                    .open(&open_spec.as_vec(), hal::Features::empty())
                    .context("Error opening logical device")?
            };

            (Arc::new(RwLock::new(gpu.device)), gpu.queue_groups)
        };

        let mut queue_negotiator = queue_negotiator.finish(queue_groups);

        // Context properties
        let properties = ContextProperties::find_best(&adapter, &surface)
            .context("Error getting context properties")?;

        // Lock device
        let mut device = device_lock
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        debug!("Detected swapchain properties: {:?}", properties);

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

        // Unlock device
        drop(device);

        let queue = queue_negotiator
            .get_queue::<DrawQueue>()
            .context("Error getting draw queue")?;

        Ok(RenderingContext {
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
        })
    }

    /// If this function fails the whole context is probably dead
    /// # Safety
    /// The context must not be used while this is being called
    pub unsafe fn handle_surface_change(&mut self) -> Result<()> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        device
            .wait_idle()
            .context("Error waiting for device to become idle")?;

        let surface = ManuallyDrop::into_inner(read(&self.target_chain))
            .deactivate_with_recyling(&mut device, &mut self.cmd_pool);

        self.properties = ContextProperties::find_best(&self.adapter, &surface)
            .context("Error finding best swapchain properties")?;

        self.target_chain = ManuallyDrop::new(
            TargetChain::new(&mut device, surface, &mut self.cmd_pool, &self.properties)
                .context("Error creating target chain")?,
        );
        Ok(())
    }

    /// Draw onto the next frame of the swapchain
    pub fn draw_next_frame<DP: DrawPass<Singular>>(
        &mut self,
        session: &Session,
        dp: &mut DP,
    ) -> Result<()> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;
        let mut queue = self
            .queue
            .write()
            .map_err(|_| LockPoisoned::Queue)
            .context("Error getting draw queue lock")?;

        // Level draw pass
        self.target_chain
            .do_draw_with(&mut device, &mut queue, dp, session)
            .context("Error preparing next target")?;

        Ok(())
    }

    /// Get a reference to the rendering context's pixels per point.
    pub fn pixels_per_point(&self) -> f32 {
        self.pixels_per_point
    }

    /// Get a reference to the rendering context's device.
    pub fn device(&self) -> &Arc<RwLock<DeviceT>> {
        &self.device
    }

    /// Get a reference to the rendering context's target chain.
    pub fn target_chain(&self) -> &TargetChain {
        &self.target_chain
    }

    /// Get a reference to the rendering context's adapter.
    pub fn adapter(&self) -> &Adapter {
        &self.adapter
    }

    /// Get a mutable reference to the rendering context's queue negotiator.
    pub fn queue_negotiator_mut(&mut self) -> &mut QueueNegotiator {
        &mut self.queue_negotiator
    }

    /// Get a reference to the physical device's properties.
    pub fn physical_device_properties(&self) -> &PhysicalDeviceProperties {
        &self.physical_device_properties
    }

    /// Get the specified memory pool, lazily initialising it if it's not yet present
    pub fn memory_pool<P: MemoryPool>(&mut self) -> Result<&Arc<RwLock<P>>> {
        self.ensure_memory_pool::<P>()?;
        Ok(self.existing_memory_pool::<P>().unwrap())
    }

    /// Ensure the specified memory pool is initialised.
    #[allow(clippy::map_entry)] // We can't follow the suggestion because of a borrowing issue
    pub fn ensure_memory_pool<P: MemoryPool>(&mut self) -> Result<()> {
        let tid = TypeId::of::<P>();
        if !self.memory_pools.contains_key(&tid) {
            self.memory_pools
                .insert(tid, Box::new(P::from_context(self)?));
        }
        Ok(())
    }

    /// Get the specified memory pool, returning None if it's not yet present
    /// You should only use this when you're certain it exists, such as when freeing memory
    /// allocated from that pool
    pub fn existing_memory_pool<P: MemoryPool>(&self) -> Option<&Arc<RwLock<P>>> {
        self.memory_pools
            .get(&TypeId::of::<P>())
            .map(|x| x.downcast_ref().unwrap())
    }

    /// Get a reference to the rendering context's properties.
    pub fn properties(&self) -> &ContextProperties {
        &self.properties
    }
}

impl core::ops::Drop for RenderingContext {
    fn drop(&mut self) {
        {
            self.device.write().unwrap().wait_idle().unwrap();
        }

        // TODO: Better deactivation code

        unsafe {
            let mut device = self.device.write().unwrap();

            ManuallyDrop::into_inner(read(&self.target_chain)).deactivate(
                &mut self.instance,
                &mut device,
                &mut self.cmd_pool,
            );

            device.destroy_command_pool(ManuallyDrop::into_inner(read(&self.cmd_pool)));
        }
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
