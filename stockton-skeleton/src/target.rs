//! Resources needed for drawing on the screen, including sync objects

use std::{
    borrow::Borrow,
    iter::{empty, once},
    mem::ManuallyDrop,
};

use hal::{
    command::CommandBufferFlags,
    format::{ChannelType, Format, ImageFeature},
    image::{Extent, FramebufferAttachment, Usage as ImgUsage, ViewCapabilities},
    pso::Viewport,
    window::{CompositeAlphaMode, Extent2D, PresentMode, SwapchainConfig},
};

use super::draw_passes::DrawPass;
use crate::{draw_passes::Singular, error::EnvironmentError, types::*};
use anyhow::{Context, Result};
use stockton_types::Session;

#[derive(Debug, Clone)]
pub struct SwapchainProperties {
    pub format: Format,
    pub depth_format: Format,
    pub present_mode: PresentMode,
    pub composite_alpha_mode: CompositeAlphaMode,
    pub viewport: Viewport,
    pub extent: Extent,
    pub image_count: u32,
}

impl SwapchainProperties {
    pub fn find_best(
        adapter: &Adapter,
        surface: &SurfaceT,
    ) -> Result<SwapchainProperties, EnvironmentError> {
        let caps = surface.capabilities(&adapter.physical_device);
        let formats = surface.supported_formats(&adapter.physical_device);

        // Find which settings we'll actually use based on preset preferences
        let format = match formats {
            Some(formats) => formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .copied()
                .ok_or(EnvironmentError::ColorFormat),
            None => Ok(Format::Rgba8Srgb),
        }?;

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

        Ok(SwapchainProperties {
            format,
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

    pub fn framebuffer_attachment(&self) -> FramebufferAttachment {
        FramebufferAttachment {
            usage: ImgUsage::COLOR_ATTACHMENT,
            format: self.format,
            view_caps: ViewCapabilities::empty(),
        }
    }
}

pub struct TargetChain {
    /// Surface we're targeting
    surface: ManuallyDrop<SurfaceT>,
    properties: SwapchainProperties,

    /// Resources tied to each target frame in the swapchain
    targets: Box<[TargetResources]>,

    /// Sync objects used in drawing
    /// These are seperated from the targets because we don't necessarily always match up indexes
    sync_objects: Box<[SyncObjects]>,

    /// The last set of sync objects used
    last_syncs: usize,

    /// Last image index of the swapchain drawn to
    last_image: u32,
}

impl TargetChain {
    pub fn new(
        device: &mut DeviceT,
        adapter: &Adapter,
        mut surface: SurfaceT,
        cmd_pool: &mut CommandPoolT,
        properties: SwapchainProperties,
    ) -> Result<TargetChain> {
        let caps = surface.capabilities(&adapter.physical_device);

        // Number of frames to pre-render
        let image_count = if properties.present_mode == PresentMode::MAILBOX {
            ((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(3))
        } else {
            ((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(2))
        };

        // Swap config
        let swap_config = SwapchainConfig {
            present_mode: properties.present_mode,
            composite_alpha_mode: properties.composite_alpha_mode,
            format: properties.format,
            extent: Extent2D {
                width: properties.extent.width,
                height: properties.extent.height,
            },
            image_count,
            image_layers: 1,
            image_usage: ImgUsage::COLOR_ATTACHMENT,
        };

        let _fat = swap_config.framebuffer_attachment();
        let mut targets: Vec<TargetResources> =
            Vec::with_capacity(swap_config.image_count as usize);
        let mut sync_objects: Vec<SyncObjects> =
            Vec::with_capacity(swap_config.image_count as usize);

        for _ in 0..swap_config.image_count {
            targets.push(
                TargetResources::new(device, cmd_pool, &properties)
                    .context("Error creating target resources")?,
            );

            sync_objects.push(SyncObjects::new(device).context("Error creating sync objects")?);
        }

        // Configure Swapchain
        unsafe {
            surface
                .configure_swapchain(device, swap_config)
                .context("Error configuring swapchain")?;
        }

        Ok(TargetChain {
            surface: ManuallyDrop::new(surface),
            targets: targets.into_boxed_slice(),
            sync_objects: sync_objects.into_boxed_slice(),
            properties,
            last_syncs: (image_count - 1) as usize, // This means the next one to be used is index 0
            last_image: 0,
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
            for i in 0..self.targets.len() {
                read(&self.targets[i]).deactivate(device, cmd_pool);
            }

            for i in 0..self.sync_objects.len() {
                read(&self.sync_objects[i]).deactivate(device);
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
        self.last_syncs = (self.last_syncs + 1) % self.sync_objects.len();
        self.last_image = (self.last_image + 1) % self.targets.len() as u32;

        let syncs = &mut self.sync_objects[self.last_syncs];
        let target = &mut self.targets[self.last_image as usize];

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
            target.cmd_buffer.begin_primary(CommandBufferFlags::empty());

            dp.queue_draw(session, img.borrow(), &mut target.cmd_buffer)
                .context("Error in draw pass")?;

            target.cmd_buffer.finish();
        }

        // Submit it
        unsafe {
            command_queue.submit(
                once(&*target.cmd_buffer),
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

    /// Get a reference to the target chain's properties.
    pub fn properties(&self) -> &SwapchainProperties {
        &self.properties
    }
}

/// Resources for a single target frame, including sync objects
pub struct TargetResources {
    /// Command buffer to use when drawing
    pub cmd_buffer: ManuallyDrop<CommandBufferT>,
}

impl TargetResources {
    pub fn new(
        _device: &mut DeviceT,
        cmd_pool: &mut CommandPoolT,
        _properties: &SwapchainProperties,
    ) -> Result<TargetResources> {
        // Command Buffer
        let cmd_buffer = unsafe { cmd_pool.allocate_one(hal::command::Level::Primary) };

        Ok(TargetResources {
            cmd_buffer: ManuallyDrop::new(cmd_buffer),
        })
    }

    pub fn deactivate(self, _device: &mut DeviceT, cmd_pool: &mut CommandPoolT) {
        use core::ptr::read;
        unsafe {
            cmd_pool.free(once(ManuallyDrop::into_inner(read(&self.cmd_buffer))));
        }
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
