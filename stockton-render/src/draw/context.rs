/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

//! Deals with all the Vulkan/HAL details.
//! In the end, this takes in a depth-sorted list of faces and a map file and renders them.
//! You'll need something else to actually find/sort the faces though.

use std::{mem::ManuallyDrop, ops::Deref};

use arrayvec::ArrayVec;
use hal::{pool::CommandPoolCreateFlags, prelude::*};
use log::debug;
use na::Mat4;
use winit::window::Window;

use super::{
    buffer::ModifiableBuffer,
    draw_buffers::DrawBuffers,
    pipeline::CompletePipeline,
    render::do_render,
    target::{SwapchainProperties, TargetChain},
    texture::TextureStore,
    ui::{do_render as do_render_ui, UIPipeline},
};
use crate::{error, types::*};
use stockton_levels::prelude::*;

/// Contains all the hal related stuff.
/// In the end, this takes in a depth-sorted list of faces and a map file and renders them.
// TODO: Settings for clear colour, buffer sizes, etc
pub struct RenderingContext<'a> {
    // Parents for most of these things
    /// Vulkan Instance
    instance: ManuallyDrop<back::Instance>,

    /// Device we're using
    device: ManuallyDrop<Device>,

    /// Adapter we're using
    adapter: Adapter,

    // Render destination
    /// Surface to draw to
    surface: ManuallyDrop<Surface>,

    /// Swapchain and stuff
    pub(crate) target_chain: ManuallyDrop<TargetChain>,

    /// Graphics pipeline and associated objects
    pipeline: ManuallyDrop<CompletePipeline>,

    /// 2D Graphics pipeline and associated objects
    ui_pipeline: ManuallyDrop<UIPipeline>,

    // Command pool and buffers
    /// The command pool used for our buffers
    cmd_pool: ManuallyDrop<CommandPool>,

    /// The queue group our buffers belong to
    queue_group: QueueGroup,

    /// Texture store
    texture_store: ManuallyDrop<TextureStore>,

    /// Buffers used for drawing
    draw_buffers: ManuallyDrop<DrawBuffers<'a>>,

    /// Buffers used for drawing the UI
    ui_draw_buffers: ManuallyDrop<DrawBuffers<'a>>,

    /// View projection matrix
    pub(crate) vp_matrix: Mat4,
}

impl<'a> RenderingContext<'a> {
    /// Create a new RenderingContext for the given window.
    pub fn new<T: HasTextures>(window: &Window, file: &T) -> Result<Self, error::CreationError> {
        // Create surface
        let (instance, mut surface, mut adapters) = unsafe {
            use hal::Instance;

            let instance = back::Instance::create("stockton", 1)
                .map_err(|_| error::CreationError::WindowError)?;
            let surface = instance
                .create_surface(window)
                .map_err(|_| error::CreationError::WindowError)?;
            let adapters = instance.enumerate_adapters();

            (instance, surface, adapters)
        };

        // TODO: Properly figure out which adapter to use
        let mut adapter = adapters.remove(0);

        // Device & Queue group
        let (mut device, mut queue_group) = {
            let family = adapter
                .queue_families
                .iter()
                .find(|family| {
                    surface.supports_queue_family(family) && family.queue_type().supports_graphics()
                })
                .unwrap();

            let mut gpu = unsafe {
                adapter
                    .physical_device
                    .open(&[(family, &[1.0])], hal::Features::empty())
                    .unwrap()
            };

            (gpu.device, gpu.queue_groups.pop().unwrap())
        };

        // Figure out what our swapchain will look like
        let swapchain_properties = SwapchainProperties::find_best(&adapter, &surface)
            .map_err(|_| error::CreationError::BadSurface)?;

        debug!(
            "Detected following swapchain properties: {:?}",
            swapchain_properties
        );

        // Command pool
        let mut cmd_pool = unsafe {
            device.create_command_pool(queue_group.family, CommandPoolCreateFlags::RESET_INDIVIDUAL)
        }
        .map_err(|_| error::CreationError::OutOfMemoryError)?;

        // Vertex and index buffers
        let draw_buffers = DrawBuffers::new(&mut device, &adapter)?;

        // UI Vertex and index buffers
        let ui_draw_buffers = DrawBuffers::new(&mut device, &adapter)?;

        // Texture store
        let texture_store = TextureStore::new(
            &mut device,
            &mut adapter,
            &mut queue_group.queues[0],
            &mut cmd_pool,
            file,
        )?;

        let mut descriptor_set_layouts: ArrayVec<[_; 2]> = ArrayVec::new();
        descriptor_set_layouts.push(texture_store.descriptor_set_layout.deref());

        // Graphics pipeline
        let pipeline = CompletePipeline::new(
            &mut device,
            swapchain_properties.extent,
            &swapchain_properties,
            descriptor_set_layouts,
        )?;

        // UI pipeline
        let ui_pipeline = UIPipeline::new(
            &mut device,
            swapchain_properties.extent,
            &swapchain_properties,
            &[],
        )?;

        // Swapchain and associated resources
        let target_chain = TargetChain::new(
            &mut device,
            &adapter,
            &mut surface,
            &pipeline,
            &ui_pipeline,
            &mut cmd_pool,
            swapchain_properties,
            None,
        )
        .map_err(error::CreationError::TargetChainCreationError)?;

        Ok(RenderingContext {
            instance: ManuallyDrop::new(instance),
            surface: ManuallyDrop::new(surface),

            device: ManuallyDrop::new(device),
            adapter,
            queue_group,

            target_chain: ManuallyDrop::new(target_chain),
            cmd_pool: ManuallyDrop::new(cmd_pool),

            pipeline: ManuallyDrop::new(pipeline),
            ui_pipeline: ManuallyDrop::new(ui_pipeline),

            texture_store: ManuallyDrop::new(texture_store),

            draw_buffers: ManuallyDrop::new(draw_buffers),
            ui_draw_buffers: ManuallyDrop::new(ui_draw_buffers),

            vp_matrix: Mat4::identity(),
        })
    }

    /// If this function fails the whole context is probably dead
    /// # Safety
    /// The context must not be used while this is being called
    pub unsafe fn handle_surface_change(&mut self) -> Result<(), error::CreationError> {
        self.device.wait_idle().unwrap();

        let properties = SwapchainProperties::find_best(&self.adapter, &self.surface)
            .map_err(|_| error::CreationError::BadSurface)?;

        use core::ptr::read;

        // Graphics pipeline
        // TODO: Recycle
        ManuallyDrop::into_inner(read(&self.pipeline)).deactivate(&mut self.device);
        self.pipeline = ManuallyDrop::new({
            let mut descriptor_set_layouts: ArrayVec<[_; 2]> = ArrayVec::new();
            descriptor_set_layouts.push(self.texture_store.descriptor_set_layout.deref());

            CompletePipeline::new(
                &mut self.device,
                properties.extent,
                &properties,
                descriptor_set_layouts,
            )?
        });

        // 2D Graphics pipeline
        // TODO: Recycle
        ManuallyDrop::into_inner(read(&self.ui_pipeline)).deactivate(&mut self.device);
        self.ui_pipeline = ManuallyDrop::new({
            UIPipeline::new(&mut self.device, properties.extent, &properties, &[])?
        });

        let old_swapchain = ManuallyDrop::into_inner(read(&self.target_chain))
            .deactivate_with_recyling(&mut self.device, &mut self.cmd_pool);
        self.target_chain = ManuallyDrop::new(
            TargetChain::new(
                &mut self.device,
                &self.adapter,
                &mut self.surface,
                &self.pipeline,
                &self.ui_pipeline,
                &mut self.cmd_pool,
                properties,
                Some(old_swapchain),
            )
            .map_err(error::CreationError::TargetChainCreationError)?,
        );

        Ok(())
    }

    /// Draw all vertices in the buffer
    pub fn draw_vertices<M: MinBSPFeatures<VulkanSystem>>(
        &mut self,
        file: &M,
        faces: &[u32],
    ) -> Result<(), &'static str> {
        // 3D Pass
        let cmd_buffer = self.target_chain.prep_next_target(
            &mut self.device,
            &mut self.draw_buffers,
            &self.pipeline,
            &self.vp_matrix,
        )?;
        do_render(
            cmd_buffer,
            &mut self.draw_buffers,
            &self.texture_store,
            &self.pipeline.pipeline_layout,
            file,
            faces,
        );

        // 2D Pass
        let cmd_buffer = self
            .target_chain
            .target_2d_pass(&mut self.ui_draw_buffers, &self.ui_pipeline)?;
        do_render_ui(cmd_buffer, &mut self.ui_draw_buffers);

        // Update our buffers before we actually start drawing
        self.draw_buffers.vertex_buffer.commit(
            &self.device,
            &mut self.queue_group.queues[0],
            &mut self.cmd_pool,
        );

        self.draw_buffers.index_buffer.commit(
            &self.device,
            &mut self.queue_group.queues[0],
            &mut self.cmd_pool,
        );

        self.ui_draw_buffers.vertex_buffer.commit(
            &self.device,
            &mut self.queue_group.queues[0],
            &mut self.cmd_pool,
        );

        self.ui_draw_buffers.index_buffer.commit(
            &self.device,
            &mut self.queue_group.queues[0],
            &mut self.cmd_pool,
        );

        // Send commands off to GPU
        self.target_chain
            .finish_and_submit_target(&mut self.queue_group.queues[0])?;

        Ok(())
    }
}

impl<'a> core::ops::Drop for RenderingContext<'a> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            use core::ptr::read;

            ManuallyDrop::into_inner(read(&self.draw_buffers)).deactivate(&mut self.device);
            ManuallyDrop::into_inner(read(&self.ui_draw_buffers)).deactivate(&mut self.device);
            ManuallyDrop::into_inner(read(&self.texture_store)).deactivate(&mut self.device);

            ManuallyDrop::into_inner(read(&self.target_chain))
                .deactivate(&mut self.device, &mut self.cmd_pool);

            self.device
                .destroy_command_pool(ManuallyDrop::into_inner(read(&self.cmd_pool)));

            ManuallyDrop::into_inner(read(&self.pipeline)).deactivate(&mut self.device);
            ManuallyDrop::into_inner(read(&self.ui_pipeline)).deactivate(&mut self.device);

            self.instance
                .destroy_surface(ManuallyDrop::into_inner(read(&self.surface)));

            ManuallyDrop::drop(&mut self.device);
        }
    }
}
