//! Resources needed for drawing on the screen, including sync objects
//! You likely won't need to interact with this directly

use crate::{
    context::ContextProperties,
    draw_passes::{DrawPass, Singular},
    types::*,
    session::Session
};

use std::{
    borrow::Borrow,
    iter::{empty, once},
    mem::ManuallyDrop,
};

use hal::{
    command::CommandBufferFlags,
    image::Usage as ImgUsage,
    window::{Extent2D, SwapchainConfig},
};

use anyhow::{Context, Result};

/// Holds our swapchain and other resources for drawing each frame
pub struct TargetChain {
    /// Surface we're targeting
    surface: ManuallyDrop<SurfaceT>,

    /// Command buffers and sync objects used when drawing
    resources: Box<[(CommandBufferT, SyncObjects)]>,

    /// Last image index of the swapchain drawn to
    last_resources: usize,
}

impl TargetChain {
    pub fn new(
        device: &mut DeviceT,
        mut surface: SurfaceT,
        cmd_pool: &mut CommandPoolT,
        properties: &ContextProperties,
    ) -> Result<TargetChain> {
        // Create swapchain
        let swap_config = SwapchainConfig {
            present_mode: properties.present_mode,
            composite_alpha_mode: properties.composite_alpha_mode,
            format: properties.color_format,
            extent: Extent2D {
                width: properties.extent.width,
                height: properties.extent.height,
            },
            image_count: properties.image_count,
            image_layers: 1,
            image_usage: ImgUsage::COLOR_ATTACHMENT,
        };

        // Create command buffers and sync objects
        let mut resources = Vec::with_capacity(swap_config.image_count as usize);

        for _ in 0..swap_config.image_count {
            resources.push((
                unsafe { cmd_pool.allocate_one(hal::command::Level::Primary) },
                SyncObjects::new(device).context("Error creating sync objects")?,
            ));
        }

        // Configure Swapchain
        unsafe {
            surface
                .configure_swapchain(device, swap_config)
                .context("Error configuring swapchain")?;
        }

        Ok(TargetChain {
            surface: ManuallyDrop::new(surface),
            resources: resources.into_boxed_slice(),
            last_resources: (properties.image_count - 1) as usize, // This means the next one to be used is index 0
        })
    }

    pub fn deactivate(
        self,
        instance: &mut InstanceT,
        device: &mut DeviceT,
        cmd_pool: &mut CommandPoolT,
    ) {
        let surface = self.deactivate_with_recyling(device, cmd_pool);

        unsafe {
            instance.destroy_surface(surface);
        }
    }

    pub fn deactivate_with_recyling(
        mut self,
        device: &mut DeviceT,
        cmd_pool: &mut CommandPoolT,
    ) -> SurfaceT {
        use core::ptr::read;
        unsafe {
            for i in 0..self.resources.len() {
                let (cmd_buf, syncs) = read(&self.resources[i]);
                cmd_pool.free(once(cmd_buf));
                syncs.deactivate(device);
            }

            self.surface.unconfigure_swapchain(device);
        }

        unsafe { ManuallyDrop::into_inner(read(&self.surface)) }
    }

    pub fn do_draw_with<'a, DP: DrawPass<Singular>>(
        &'a mut self,
        device: &mut DeviceT,
        command_queue: &mut QueueT,
        dp: &mut DP,
        session: &Session,
    ) -> Result<()> {
        self.last_resources = (self.last_resources + 1) % self.resources.len();

        let (cmd_buffer, syncs) = &mut self.resources[self.last_resources];

        // Get the image
        let (img, _) = unsafe {
            self.surface
                .acquire_image(core::u64::MAX)
                .context("Error getting image from swapchain")?
        };

        // Make sure whatever was last using this has finished
        unsafe {
            device
                .wait_for_fence(&syncs.present_complete, core::u64::MAX)
                .context("Error waiting for present_complete")?;
            device
                .reset_fence(&mut syncs.present_complete)
                .context("Error resetting present_complete fence")?;
        };

        // Record commands
        unsafe {
            cmd_buffer.begin_primary(CommandBufferFlags::empty());

            dp.queue_draw(session, img.borrow(), cmd_buffer)
                .context("Error in draw pass")?;

            cmd_buffer.finish();
        }

        // Submit it
        unsafe {
            command_queue.submit(
                once(&*cmd_buffer),
                empty(),
                once(&*syncs.render_complete),
                Some(&mut syncs.present_complete),
            );
            command_queue
                .present(&mut self.surface, img, Some(&mut *syncs.render_complete))
                .context("Error presenting to surface")?;
        };

        Ok(())
    }
}

pub struct SyncObjects {
    /// Triggered when rendering is done
    pub render_complete: ManuallyDrop<SemaphoreT>,

    /// Triggered when the image is on screen
    pub present_complete: ManuallyDrop<FenceT>,
}

impl SyncObjects {
    pub fn new(device: &mut DeviceT) -> Result<Self> {
        // Sync objects
        let render_complete = device
            .create_semaphore()
            .context("Error creating render_complete semaphore")?;
        let present_complete = device
            .create_fence(true)
            .context("Error creating present_complete fence")?;

        Ok(SyncObjects {
            render_complete: ManuallyDrop::new(render_complete),
            present_complete: ManuallyDrop::new(present_complete),
        })
    }

    pub fn deactivate(self, device: &mut DeviceT) {
        use core::ptr::read;

        unsafe {
            device.destroy_semaphore(ManuallyDrop::into_inner(read(&self.render_complete)));
            device.destroy_fence(ManuallyDrop::into_inner(read(&self.present_complete)));
        }
    }
}
