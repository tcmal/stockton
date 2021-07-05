//! Deals with all the Vulkan/HAL details.
//! This relies on draw passes for the actual drawing logic.

use std::{
    mem::ManuallyDrop,
    ptr::read,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};
use hal::pool::CommandPoolCreateFlags;
use log::debug;
use na::Mat4;
use winit::window::Window;

use super::{
    draw_passes::{DrawPass, IntoDrawPass, LevelDrawPass},
    queue_negotiator::{DrawQueue, QueueNegotiator},
    target::{SwapchainProperties, TargetChain},
};
use crate::{
    error::{EnvironmentError, LockPoisoned},
    types::*,
    window::UiState,
};
use stockton_levels::prelude::*;
use stockton_types::Session;

/// Contains all the hal related stuff.
/// In the end, this takes in a depth-sorted list of faces and a map file and renders them.
// TODO: Settings for clear colour, buffer sizes, etc
pub struct RenderingContext<DP> {
    // Parents for most of these things
    /// Vulkan Instance
    instance: ManuallyDrop<back::Instance>,

    /// Device we're using
    device: Arc<RwLock<DeviceT>>,

    /// Adapter we're using
    adapter: Adapter,

    /// Swapchain and stuff
    pub(crate) target_chain: ManuallyDrop<TargetChain>,

    // Command pool and buffers
    /// The command pool used for our buffers
    cmd_pool: ManuallyDrop<CommandPoolT>,

    /// The queue to use for drawing
    queue: Arc<RwLock<QueueT>>,

    /// Deals with drawing logic, and holds any data required for drawing.
    draw_pass: ManuallyDrop<DP>,

    /// View projection matrix
    pub(crate) vp_matrix: Mat4,

    pub(crate) pixels_per_point: f32,
}

impl<DP: DrawPass> RenderingContext<DP> {
    /// Create a new RenderingContext for the given window.
    pub fn new<ILDP: IntoDrawPass<DP>>(
        window: &Window,
        idp: ILDP,
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
        let mut queue_families_specs = Vec::new();
        let (mut queue_negotiator, surface) = {
            let dq: DrawQueue = DrawQueue { surface };

            let mut qn = QueueNegotiator::new();

            // Draw Queue
            qn.find(&adapter, &dq)
                .context("Couldn't find draw queue family")?;
            queue_families_specs.push(
                qn.family_spec::<DrawQueue>(&adapter.queue_families, 1)
                    .context("Couldn't find draw queue family")?,
            );

            // Auxiliary queues for DP
            queue_families_specs.extend(
                DP::find_aux_queues(&adapter, &mut qn)
                    .context("Level pass couldn't populate queue negotiator")?,
            );

            (qn, dq.surface)
        };

        // Device & Queue groups
        let (device_lock, mut queue_groups) = {
            // TODO: This sucks, but hal is restrictive on how we can pass this specific argument.
            let queue_families_specs_real: Vec<_> = queue_families_specs
                .iter()
                .map(|(qf, ns)| (*qf, ns.as_slice()))
                .collect();

            let gpu = unsafe {
                adapter
                    .physical_device
                    .open(queue_families_specs_real.as_slice(), hal::Features::empty())
                    .context("Error opening logical device")?
            };

            (Arc::new(RwLock::new(gpu.device)), gpu.queue_groups)
        };

        // Figure out what our swapchain will look like
        let swapchain_properties = SwapchainProperties::find_best(&adapter, &surface)
            .context("Error getting properties for swapchain")?;

        // Draw pass
        let dp = idp.init(
            device_lock.clone(),
            &mut queue_negotiator,
            &swapchain_properties,
        )?;

        // Lock device
        let mut device = device_lock
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        debug!("Detected swapchain properties: {:?}", swapchain_properties);

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
        let target_chain = TargetChain::new(
            &mut device,
            &adapter,
            surface,
            &mut cmd_pool,
            swapchain_properties,
        )
        .context("Error creating target chain")?;

        // Unlock device
        drop(device);

        Ok(RenderingContext {
            instance: ManuallyDrop::new(instance),

            device: device_lock,
            adapter,

            queue: queue_negotiator
                .get_queue::<DrawQueue>(&mut queue_groups)
                .ok_or(EnvironmentError::NoQueues)
                .context("Error getting draw queue")?,

            draw_pass: ManuallyDrop::new(dp),
            target_chain: ManuallyDrop::new(target_chain),
            cmd_pool: ManuallyDrop::new(cmd_pool),

            vp_matrix: Mat4::identity(),

            // pixels_per_point: window.scale_factor() as f32,
            pixels_per_point: window.scale_factor() as f32,
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

        let properties = SwapchainProperties::find_best(&self.adapter, &surface)
            .context("Error finding best swapchain properties")?;

        // TODO: Notify draw passes

        self.target_chain = ManuallyDrop::new(
            TargetChain::new(
                &mut device,
                &self.adapter,
                surface,
                &mut self.cmd_pool,
                properties,
            )
            .context("Error creating target chain")?,
        );
        Ok(())
    }

    /// Draw onto the next frame of the swapchain
    pub fn draw_next_frame(&mut self, session: &Session) -> Result<()> {
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
            .do_draw_with(&mut device, &mut queue, &*self.draw_pass, session)
            .context("Error preparing next target")?;

        Ok(())
    }
}

impl<DP> core::ops::Drop for RenderingContext<DP> {
    fn drop(&mut self) {
        {
            self.device.write().unwrap().wait_idle().unwrap();
        }

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
