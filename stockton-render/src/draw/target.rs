// Copyright (C) Oscar Shrimpton 2019

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Resources needed for drawing on the screen, including sync objects
use super::{camera::WorkingCamera, texture::image::LoadedImage};
use crate::types::*;

use core::{iter::once, mem::ManuallyDrop};

use arrayvec::ArrayVec;
use draw::buffer::ModifiableBuffer;
use draw::draw_buffers::DrawBuffers;
use hal::{
    format::{ChannelType, Format, Swizzle},
    image::{Extent, Usage as ImgUsage, ViewKind},
    prelude::*,
    pso::Viewport,
    queue::Submission,
    window::{CompositeAlphaMode, Extent2D, PresentMode, SwapchainConfig},
};

/// Defines the colour range we use.
const COLOR_RANGE: hal::image::SubresourceRange = hal::image::SubresourceRange {
    aspects: hal::format::Aspects::COLOR,
    levels: 0..1,
    layers: 0..1,
};

#[derive(Debug, Clone)]
pub struct SwapchainProperties {
    pub format: Format,
    pub depth_format: Format,
    pub present_mode: PresentMode,
    pub composite_alpha_mode: CompositeAlphaMode,
    pub viewport: Viewport,
    pub extent: Extent,
}

impl SwapchainProperties {
    pub fn find_best(adapter: &Adapter, surface: &Surface) -> Result<SwapchainProperties, ()> {
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
        .ok_or(())?;

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
            .ok_or(())?
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
            .ok_or(())?
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
    /// Swapchain we're targeting
    pub swapchain: ManuallyDrop<Swapchain>,

    pub properties: SwapchainProperties,

    /// The depth buffer/image used for drawing
    pub depth_buffer: ManuallyDrop<LoadedImage>,

    /// Resources tied to each target frame in the swapchain
    pub targets: Box<[TargetResources]>,

    /// The last target drawn to
    last_drawn: usize,

    /// Last image index of the swapchain drawn to
    last_image_index: u32,
}

impl TargetChain {
    pub fn new(
        device: &mut Device,
        adapter: &Adapter,
        surface: &mut Surface,
        renderpass: &RenderPass,
        cmd_pool: &mut CommandPool,
        properties: SwapchainProperties,
        old_swapchain: Option<Swapchain>,
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

        // Swapchain
        let (swapchain, mut backbuffer) = unsafe {
            device
                .create_swapchain(surface, swap_config, old_swapchain)
                .map_err(|_| TargetChainCreationError::Todo)?
        };

        let depth_buffer: LoadedImage = {
            use hal::format::Aspects;
            use hal::image::SubresourceRange;

            LoadedImage::new(
                device,
                adapter,
                properties.depth_format,
                ImgUsage::DEPTH_STENCIL_ATTACHMENT,
                SubresourceRange {
                    aspects: Aspects::DEPTH,
                    levels: 0..1,
                    layers: 0..1,
                },
                properties.extent.width as usize,
                properties.extent.height as usize,
            )
            .map_err(|_| TargetChainCreationError::Todo)
        }?;

        let mut targets: Vec<TargetResources> = Vec::with_capacity(backbuffer.len());
        for image in backbuffer.drain(..) {
            targets.push(
                TargetResources::new(
                    device,
                    cmd_pool,
                    renderpass,
                    image,
                    &(*depth_buffer.image_view),
                    properties.extent,
                    properties.format,
                )
                .map_err(|_| TargetChainCreationError::Todo)?,
            );
        }

        Ok(TargetChain {
            swapchain: ManuallyDrop::new(swapchain),
            targets: targets.into_boxed_slice(),
            depth_buffer: ManuallyDrop::new(depth_buffer),
            properties,
            last_drawn: (image_count - 1) as usize, // This means the next one to be used is index 0
            last_image_index: 0,
        })
    }

    pub fn deactivate(self, device: &mut Device, cmd_pool: &mut CommandPool) {
        use core::ptr::read;
        unsafe {
            ManuallyDrop::into_inner(read(&self.depth_buffer)).deactivate(device);

            for i in 0..self.targets.len() {
                read(&self.targets[i]).deactivate(device, cmd_pool);
            }

            device.destroy_swapchain(ManuallyDrop::into_inner(read(&self.swapchain)));
        }
    }

    pub fn deactivate_with_recyling(
        self,
        device: &mut Device,
        cmd_pool: &mut CommandPool,
    ) -> Swapchain {
        use core::ptr::read;
        unsafe {
            ManuallyDrop::into_inner(read(&self.depth_buffer)).deactivate(device);

            for i in 0..self.targets.len() {
                read(&self.targets[i]).deactivate(device, cmd_pool);
            }
        }

        unsafe { ManuallyDrop::into_inner(read(&self.swapchain)) }
    }

    pub fn prep_next_target<'a>(
        &'a mut self,
        device: &mut Device,
        draw_buffers: &mut DrawBuffers,
        renderpass: &RenderPass,
        pipeline: &GraphicsPipeline,
        pipeline_layout: &PipelineLayout,
        camera: &mut WorkingCamera,
    ) -> Result<&'a mut crate::types::CommandBuffer, &'static str> {
        self.last_drawn = (self.last_drawn + 1) % self.targets.len();

        let target = &mut self.targets[self.last_drawn];

        // Get the image
        let (image_index, _) = unsafe {
            self.swapchain
                .acquire_image(core::u64::MAX, Some(&target.get_image), None)
                .map_err(|_| "FrameError::AcquireError")?
        };

        self.last_image_index = image_index;

        // Make sure whatever was last using this has finished
        unsafe {
            device
                .wait_for_fence(&target.present_complete, core::u64::MAX)
                .map_err(|_| "FrameError::SyncObjectError")?;
            device
                .reset_fence(&target.present_complete)
                .map_err(|_| "FrameError::SyncObjectError")?;
        };

        // Record commands
        unsafe {
            use hal::buffer::{IndexBufferView, SubRange};
            use hal::command::{
                ClearColor, ClearDepthStencil, ClearValue, CommandBufferFlags, SubpassContents,
            };
            use hal::pso::ShaderStageFlags;

            // Colour to clear window to
            let clear_values = [
                ClearValue {
                    color: ClearColor {
                        float32: [0.0, 0.0, 0.0, 1.0],
                    },
                },
                ClearValue {
                    depth_stencil: ClearDepthStencil {
                        depth: 1.0,
                        stencil: 0,
                    },
                },
            ];

            // Get references to our buffers
            let (vbufs, ibuf) = {
                let vbufref: &<back::Backend as hal::Backend>::Buffer =
                    draw_buffers.vertex_buffer.get_buffer();

                let vbufs: ArrayVec<[_; 1]> = [(vbufref, SubRange::WHOLE)].into();
                let ibuf = draw_buffers.index_buffer.get_buffer();

                (vbufs, ibuf)
            };

            target.cmd_buffer.begin_primary(CommandBufferFlags::EMPTY);
            // Main render pass / pipeline
            target.cmd_buffer.begin_render_pass(
                renderpass,
                &target.framebuffer,
                self.properties.viewport.rect,
                clear_values.iter(),
                SubpassContents::Inline,
            );
            target.cmd_buffer.bind_graphics_pipeline(&pipeline);

            // VP Matrix
            let vp = camera.get_matrix().as_slice();
            let vp = &*(vp as *const [f32] as *const [u32]);

            target.cmd_buffer.push_graphics_constants(
                &pipeline_layout,
                ShaderStageFlags::VERTEX,
                0,
                vp,
            );

            // Bind buffers
            target.cmd_buffer.bind_vertex_buffers(0, vbufs);
            target.cmd_buffer.bind_index_buffer(IndexBufferView {
                buffer: ibuf,
                range: SubRange::WHOLE,
                index_type: hal::IndexType::U16,
            });
        };

        Ok(&mut target.cmd_buffer)
    }

    pub fn finish_and_submit_target(
        &mut self,
        command_queue: &mut CommandQueue,
    ) -> Result<(), &'static str> {
        let target = &mut self.targets[self.last_drawn];

        unsafe {
            target.cmd_buffer.end_render_pass();
            target.cmd_buffer.finish();
        }

        // Make submission object
        let command_buffers: std::iter::Once<&CommandBuffer> = once(&target.cmd_buffer);
        let wait_semaphores: std::iter::Once<(&Semaphore, hal::pso::PipelineStage)> = once((
            &target.get_image,
            hal::pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT,
        ));
        let signal_semaphores: std::iter::Once<&Semaphore> = once(&target.render_complete);

        let present_wait_semaphores: std::iter::Once<&Semaphore> = once(&target.render_complete);

        let submission = Submission {
            command_buffers,
            wait_semaphores,
            signal_semaphores,
        };

        // Submit it
        unsafe {
            command_queue.submit(submission, Some(&target.present_complete));
            self.swapchain
                .present(
                    command_queue,
                    self.last_image_index as u32,
                    present_wait_semaphores,
                )
                .map_err(|_| "FrameError::PresentError")?;
        };

        // TODO
        Ok(())
    }
}

/// Resources for a single target frame, including sync objects
pub struct TargetResources {
    /// Command buffer to use when drawing
    pub cmd_buffer: ManuallyDrop<CommandBuffer>,

    /// The image for this frame
    pub image: ManuallyDrop<Image>,

    /// Imageviews for this frame
    pub imageview: ManuallyDrop<ImageView>,

    /// Framebuffer for this frame
    pub framebuffer: ManuallyDrop<Framebuffer>,

    // Sync objects
    /// Triggered when the image is ready to draw to
    pub get_image: ManuallyDrop<Semaphore>,

    /// Triggered when rendering is done
    pub render_complete: ManuallyDrop<Semaphore>,

    /// Triggered when the image is on screen
    pub present_complete: ManuallyDrop<Fence>,
}

impl TargetResources {
    pub fn new(
        device: &mut Device,
        cmd_pool: &mut CommandPool,
        renderpass: &RenderPass,
        image: Image,
        depth_pass: &ImageView,
        extent: Extent,
        format: Format,
    ) -> Result<TargetResources, TargetResourcesCreationError> {
        // Command Buffer
        let cmd_buffer = unsafe { cmd_pool.allocate_one(hal::command::Level::Primary) };

        // ImageView
        let imageview = unsafe {
            device
                .create_image_view(
                    &image,
                    ViewKind::D2,
                    format,
                    Swizzle::NO,
                    COLOR_RANGE.clone(),
                )
                .map_err(TargetResourcesCreationError::ImageViewError)?
        };

        // Framebuffer
        let framebuffer = unsafe {
            device
                .create_framebuffer(
                    &renderpass,
                    once(&imageview).chain(once(depth_pass)),
                    extent,
                )
                .map_err(|_| TargetResourcesCreationError::FrameBufferNoMemory)?
        };

        // Sync objects
        let get_image = device
            .create_semaphore()
            .map_err(|_| TargetResourcesCreationError::SyncObjectsNoMemory)?;
        let render_complete = device
            .create_semaphore()
            .map_err(|_| TargetResourcesCreationError::SyncObjectsNoMemory)?;
        let present_complete = device
            .create_fence(true)
            .map_err(|_| TargetResourcesCreationError::SyncObjectsNoMemory)?;

        Ok(TargetResources {
            cmd_buffer: ManuallyDrop::new(cmd_buffer),
            image: ManuallyDrop::new(image),
            imageview: ManuallyDrop::new(imageview),
            framebuffer: ManuallyDrop::new(framebuffer),
            get_image: ManuallyDrop::new(get_image),
            render_complete: ManuallyDrop::new(render_complete),
            present_complete: ManuallyDrop::new(present_complete),
        })
    }

    pub fn deactivate(self, device: &mut Device, cmd_pool: &mut CommandPool) {
        use core::ptr::read;
        unsafe {
            cmd_pool.free(once(ManuallyDrop::into_inner(read(&self.cmd_buffer))));

            device.destroy_framebuffer(ManuallyDrop::into_inner(read(&self.framebuffer)));
            device.destroy_image_view(ManuallyDrop::into_inner(read(&self.imageview)));

            device.destroy_semaphore(ManuallyDrop::into_inner(read(&self.get_image)));
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
    ImageViewError(hal::image::ViewCreationError),
    FrameBufferNoMemory,
    SyncObjectsNoMemory,
}
