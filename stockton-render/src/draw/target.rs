//! Resources needed for drawing on the screen, including sync objects

use std::{
    array::IntoIter,
    borrow::Borrow,
    iter::{empty, once},
    mem::ManuallyDrop,
};

use arrayvec::ArrayVec;
use hal::{
    buffer::SubRange,
    command::RenderAttachmentInfo,
    format::{ChannelType, Format},
    image::{Extent, FramebufferAttachment, Usage as ImgUsage, ViewCapabilities},
    pso::Viewport,
    window::{CompositeAlphaMode, Extent2D, PresentMode, SwapchainConfig},
};
use na::Mat4;

use super::{
    buffer::ModifiableBuffer,
    depth_buffer::DedicatedLoadedImage,
    draw_buffers::{DrawBuffers, UvPoint},
    pipeline::CompletePipeline,
    ui::{UiPipeline, UiPoint},
};
use crate::types::*;

#[derive(Debug, Clone)]
pub struct SwapchainProperties {
    pub format: Format,
    pub depth_format: Format,
    pub present_mode: PresentMode,
    pub composite_alpha_mode: CompositeAlphaMode,
    pub viewport: Viewport,
    pub extent: Extent,
}

/// Indicates the given property has no acceptable values
pub enum NoSupportedValuesError {
    DepthFormat,
    PresentMode,
    CompositeAlphaMode,
}

impl SwapchainProperties {
    pub fn find_best(
        adapter: &Adapter,
        surface: &SurfaceT,
    ) -> Result<SwapchainProperties, NoSupportedValuesError> {
        let caps = surface.capabilities(&adapter.physical_device);
        let formats = surface.supported_formats(&adapter.physical_device);

        // Find which settings we'll actually use based on preset preferences
        let format = formats.map_or(Format::Rgba8Srgb, |formats| {
            formats
                .iter()
                .find(|format| format.base_format().1 == ChannelType::Srgb)
                .copied()
                .unwrap_or(formats[0])
        });

        let depth_format = *[
            Format::D32SfloatS8Uint,
            Format::D24UnormS8Uint,
            Format::D32Sfloat,
        ]
        .iter()
        .find(|format| {
            use hal::format::ImageFeature;

            format.is_depth()
                && adapter
                    .physical_device
                    .format_properties(Some(**format))
                    .optimal_tiling
                    .contains(ImageFeature::DEPTH_STENCIL_ATTACHMENT)
        })
        .ok_or(NoSupportedValuesError::DepthFormat)?;

        let present_mode = {
            [
                PresentMode::MAILBOX,
                PresentMode::FIFO,
                PresentMode::RELAXED,
                PresentMode::IMMEDIATE,
            ]
            .iter()
            .cloned()
            .find(|pm| caps.present_modes.contains(*pm))
            .ok_or(NoSupportedValuesError::PresentMode)?
        };
        let composite_alpha_mode = {
            [
                CompositeAlphaMode::OPAQUE,
                CompositeAlphaMode::INHERIT,
                CompositeAlphaMode::PREMULTIPLIED,
                CompositeAlphaMode::POSTMULTIPLIED,
            ]
            .iter()
            .cloned()
            .find(|ca| caps.composite_alpha_modes.contains(*ca))
            .ok_or(NoSupportedValuesError::CompositeAlphaMode)?
        };

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
        })
    }
}

pub struct TargetChain {
    /// Surface we're targeting
    pub surface: ManuallyDrop<SurfaceT>,

    pub properties: SwapchainProperties,

    /// The depth buffer/image used for drawing
    pub depth_buffer: ManuallyDrop<DedicatedLoadedImage>,

    /// Resources tied to each target frame in the swapchain
    pub targets: Box<[TargetResources]>,

    /// Sync objects used in drawing
    /// These are seperated from the targets because we don't necessarily always match up indexes
    pub sync_objects: Box<[SyncObjects]>,

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
        pipeline: &CompletePipeline,
        ui_pipeline: &UiPipeline,
        cmd_pool: &mut CommandPoolT,
        properties: SwapchainProperties,
    ) -> Result<TargetChain, TargetChainCreationError> {
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

        let depth_buffer = {
            use hal::format::Aspects;
            use hal::image::SubresourceRange;

            DedicatedLoadedImage::new(
                device,
                adapter,
                properties.depth_format,
                ImgUsage::DEPTH_STENCIL_ATTACHMENT,
                SubresourceRange {
                    aspects: Aspects::DEPTH,
                    level_start: 0,
                    level_count: Some(1),
                    layer_start: 0,
                    layer_count: Some(1),
                },
                properties.extent.width as usize,
                properties.extent.height as usize,
            )
            .map_err(|_| TargetChainCreationError::Todo)
        }?;

        let fat = swap_config.framebuffer_attachment();
        let mut targets: Vec<TargetResources> =
            Vec::with_capacity(swap_config.image_count as usize);
        let mut sync_objects: Vec<SyncObjects> =
            Vec::with_capacity(swap_config.image_count as usize);
        for _ in 0..swap_config.image_count {
            targets.push(
                TargetResources::new(
                    device,
                    cmd_pool,
                    &pipeline.renderpass,
                    &ui_pipeline.renderpass,
                    fat.clone(),
                    FramebufferAttachment {
                        usage: ImgUsage::DEPTH_STENCIL_ATTACHMENT,
                        view_caps: ViewCapabilities::empty(),
                        format: properties.depth_format,
                    },
                    &properties,
                )
                .map_err(|_| TargetChainCreationError::Todo)?,
            );

            sync_objects
                .push(SyncObjects::new(device).map_err(|_| TargetChainCreationError::Todo)?);
        }

        // Configure Swapchain
        unsafe {
            surface
                .configure_swapchain(device, swap_config)
                .map_err(|_| TargetChainCreationError::Todo)?;
        }

        Ok(TargetChain {
            surface: ManuallyDrop::new(surface),
            targets: targets.into_boxed_slice(),
            sync_objects: sync_objects.into_boxed_slice(),
            depth_buffer: ManuallyDrop::new(depth_buffer),
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
            ManuallyDrop::into_inner(read(&self.depth_buffer)).deactivate(device);

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

    pub fn prep_next_target<'a>(
        &'a mut self,
        device: &mut DeviceT,
        draw_buffers: &mut DrawBuffers<UvPoint>,
        pipeline: &CompletePipeline,
        vp: &Mat4,
    ) -> Result<
        (
            &'a mut crate::types::CommandBufferT,
            <SurfaceT as PresentationSurface<back::Backend>>::SwapchainImage,
        ),
        &'static str,
    > {
        self.last_syncs = (self.last_syncs + 1) % self.sync_objects.len();

        let syncs = &mut self.sync_objects[self.last_syncs];

        self.last_image = (self.last_image + 1) % self.targets.len() as u32;

        let target = &mut self.targets[self.last_image as usize];

        // Get the image
        let (img, _) = unsafe {
            self.surface
                .acquire_image(core::u64::MAX)
                .map_err(|_| "FrameError::AcquireError")?
        };

        // Make sure whatever was last using this has finished
        unsafe {
            device
                .wait_for_fence(&syncs.present_complete, core::u64::MAX)
                .map_err(|_| "FrameError::SyncObjectError")?;
            device
                .reset_fence(&mut syncs.present_complete)
                .map_err(|_| "FrameError::SyncObjectError")?;
        };

        // Record commands
        unsafe {
            use hal::command::{
                ClearColor, ClearDepthStencil, ClearValue, CommandBufferFlags, SubpassContents,
            };
            use hal::pso::ShaderStageFlags;

            // Get references to our buffers
            let (vbufs, ibuf) = {
                let vbufref: &<back::Backend as hal::Backend>::Buffer =
                    draw_buffers.vertex_buffer.get_buffer();

                let vbufs: ArrayVec<[_; 1]> = [(
                    vbufref,
                    SubRange {
                        offset: 0,
                        size: None,
                    },
                )]
                .into();
                let ibuf = draw_buffers.index_buffer.get_buffer();

                (vbufs, ibuf)
            };

            target.cmd_buffer.begin_primary(CommandBufferFlags::empty());
            // Main render pass / pipeline
            target.cmd_buffer.begin_render_pass(
                &pipeline.renderpass,
                &target.framebuffer,
                self.properties.viewport.rect,
                vec![
                    RenderAttachmentInfo {
                        image_view: img.borrow(),
                        clear_value: ClearValue {
                            color: ClearColor {
                                float32: [0.0, 0.0, 0.0, 1.0],
                            },
                        },
                    },
                    RenderAttachmentInfo {
                        image_view: &*self.depth_buffer.image_view,
                        clear_value: ClearValue {
                            depth_stencil: ClearDepthStencil {
                                depth: 1.0,
                                stencil: 0,
                            },
                        },
                    },
                ]
                .into_iter(),
                SubpassContents::Inline,
            );
            target.cmd_buffer.bind_graphics_pipeline(&pipeline.pipeline);

            // VP Matrix
            let vp = &*(vp.data.as_slice() as *const [f32] as *const [u32]);

            target.cmd_buffer.push_graphics_constants(
                &pipeline.pipeline_layout,
                ShaderStageFlags::VERTEX,
                0,
                vp,
            );

            // Bind buffers
            target.cmd_buffer.bind_vertex_buffers(0, vbufs.into_iter());
            target.cmd_buffer.bind_index_buffer(
                &ibuf,
                SubRange {
                    offset: 0,
                    size: None,
                },
                hal::IndexType::U16,
            );
        };

        Ok((&mut target.cmd_buffer, img))
    }

    pub fn target_2d_pass<'a>(
        &'a mut self,
        draw_buffers: &mut DrawBuffers<UiPoint>,
        img: &<SurfaceT as PresentationSurface<back::Backend>>::SwapchainImage,
        pipeline: &UiPipeline,
    ) -> Result<&'a mut CommandBufferT, &'static str> {
        let target = &mut self.targets[self.last_image as usize];

        unsafe {
            use hal::pso::PipelineStage;
            target.cmd_buffer.end_render_pass();

            target.cmd_buffer.pipeline_barrier(
                PipelineStage::BOTTOM_OF_PIPE..PipelineStage::TOP_OF_PIPE,
                hal::memory::Dependencies::empty(),
                std::iter::empty(),
            );
        }

        // Record commands
        unsafe {
            use hal::command::{ClearColor, ClearValue, SubpassContents};

            // Get references to our buffers
            let (vbufs, ibuf) = {
                let vbufref: &<back::Backend as hal::Backend>::Buffer =
                    draw_buffers.vertex_buffer.get_buffer();

                let vbufs: ArrayVec<[_; 1]> = [(
                    vbufref,
                    SubRange {
                        offset: 0,
                        size: None,
                    },
                )]
                .into();
                let ibuf = draw_buffers.index_buffer.get_buffer();

                (vbufs, ibuf)
            };

            // Main render pass / pipeline
            target.cmd_buffer.begin_render_pass(
                &pipeline.renderpass,
                &target.framebuffer_2d,
                self.properties.viewport.rect,
                vec![RenderAttachmentInfo {
                    image_view: img.borrow(),
                    clear_value: ClearValue {
                        color: ClearColor {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    },
                }]
                .into_iter(),
                SubpassContents::Inline,
            );
            target.cmd_buffer.bind_graphics_pipeline(&pipeline.pipeline);

            // Bind buffers
            target.cmd_buffer.bind_vertex_buffers(0, vbufs.into_iter());
            target.cmd_buffer.bind_index_buffer(
                &ibuf,
                SubRange {
                    offset: 0,
                    size: None,
                },
                hal::IndexType::U16,
            );
        };

        Ok(&mut target.cmd_buffer)
    }

    pub fn finish_and_submit_target(
        &mut self,
        img: <SurfaceT as PresentationSurface<back::Backend>>::SwapchainImage,
        command_queue: &mut QueueT,
    ) -> Result<(), &'static str> {
        let syncs = &mut self.sync_objects[self.last_syncs];
        let target = &mut self.targets[self.last_image as usize];

        unsafe {
            target.cmd_buffer.end_render_pass();
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
                .map_err(|_| "FrameError::PresentError")?;
        };

        Ok(())
    }
}

/// Resources for a single target frame, including sync objects
pub struct TargetResources {
    /// Command buffer to use when drawing
    pub cmd_buffer: ManuallyDrop<CommandBufferT>,

    /// Framebuffer for this frame
    pub framebuffer: ManuallyDrop<FramebufferT>,

    /// Framebuffer for this frame when drawing in 2D
    pub framebuffer_2d: ManuallyDrop<FramebufferT>,
}

impl TargetResources {
    pub fn new(
        device: &mut DeviceT,
        cmd_pool: &mut CommandPoolT,
        renderpass: &RenderPassT,
        renderpass_2d: &RenderPassT,
        fat: FramebufferAttachment,
        dat: FramebufferAttachment,
        properties: &SwapchainProperties,
    ) -> Result<TargetResources, TargetResourcesCreationError> {
        // Command Buffer
        let cmd_buffer = unsafe { cmd_pool.allocate_one(hal::command::Level::Primary) };

        // Framebuffer
        let framebuffer = unsafe {
            device
                .create_framebuffer(
                    &renderpass,
                    IntoIter::new([fat.clone(), dat]),
                    properties.extent,
                )
                .map_err(|_| TargetResourcesCreationError::FrameBufferNoMemory)?
        };

        // 2D framebuffer just needs the imageview, not the depth pass
        let framebuffer_2d = unsafe {
            device
                .create_framebuffer(&renderpass_2d, once(fat), properties.extent)
                .map_err(|_| TargetResourcesCreationError::FrameBufferNoMemory)?
        };

        Ok(TargetResources {
            cmd_buffer: ManuallyDrop::new(cmd_buffer),
            framebuffer: ManuallyDrop::new(framebuffer),
            framebuffer_2d: ManuallyDrop::new(framebuffer_2d),
        })
    }

    pub fn deactivate(self, device: &mut DeviceT, cmd_pool: &mut CommandPoolT) {
        use core::ptr::read;
        unsafe {
            cmd_pool.free(once(ManuallyDrop::into_inner(read(&self.cmd_buffer))));

            device.destroy_framebuffer(ManuallyDrop::into_inner(read(&self.framebuffer)));
            device.destroy_framebuffer(ManuallyDrop::into_inner(read(&self.framebuffer_2d)));
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
    pub fn new(device: &mut DeviceT) -> Result<Self, TargetResourcesCreationError> {
        // Sync objects
        let render_complete = device
            .create_semaphore()
            .map_err(|_| TargetResourcesCreationError::SyncObjectsNoMemory)?;
        let present_complete = device
            .create_fence(true)
            .map_err(|_| TargetResourcesCreationError::SyncObjectsNoMemory)?;

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

#[derive(Debug)]
pub enum TargetChainCreationError {
    Todo,
}

#[derive(Debug)]
pub enum TargetResourcesCreationError {
    ImageViewError,
    FrameBufferNoMemory,
    SyncObjectsNoMemory,
}
