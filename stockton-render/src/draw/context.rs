//! Deals with all the Vulkan/HAL details.
//! In the end, this takes in a depth-sorted list of faces and a map file and renders them.
//! You'll need something else to actually find/sort the faces though.

use std::{
    iter::once,
    mem::ManuallyDrop,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};
use arrayvec::ArrayVec;
use hal::pool::CommandPoolCreateFlags;
use log::debug;
use na::Mat4;
use winit::window::Window;

use super::{
    buffer::ModifiableBuffer,
    draw_buffers::{DrawBuffers, UvPoint},
    pipeline::CompletePipeline,
    queue_negotiator::{DrawQueue, QueueNegotiator},
    render::do_render,
    target::{SwapchainProperties, TargetChain},
    texture::{resolver::FsResolver, TexLoadQueue, TextureLoadConfig, TextureRepo},
    ui::{
        do_render as do_render_ui, ensure_textures as ensure_textures_ui, UiPipeline, UiPoint,
        UiTextures,
    },
};
use crate::{
    error::{EnvironmentError, LockPoisoned},
    types::*,
    window::UiState,
};
use stockton_levels::prelude::*;

/// Contains all the hal related stuff.
/// In the end, this takes in a depth-sorted list of faces and a map file and renders them.
// TODO: Settings for clear colour, buffer sizes, etc
pub struct RenderingContext<'a, M: 'static + MinBspFeatures<VulkanSystem>> {
    pub map: Arc<RwLock<M>>,

    // Parents for most of these things
    /// Vulkan Instance
    instance: ManuallyDrop<back::Instance>,

    /// Device we're using
    device: Arc<RwLock<DeviceT>>,

    /// Adapter we're using
    adapter: Adapter,

    /// Swapchain and stuff
    pub(crate) target_chain: ManuallyDrop<TargetChain>,

    /// Graphics pipeline and associated objects
    pipeline: ManuallyDrop<CompletePipeline>,

    /// 2D Graphics pipeline and associated objects
    ui_pipeline: ManuallyDrop<UiPipeline>,

    // Command pool and buffers
    /// The command pool used for our buffers
    cmd_pool: ManuallyDrop<CommandPoolT>,

    /// The queue to use for drawing
    queue: Arc<RwLock<QueueT>>,

    /// Main Texture repo
    tex_repo: ManuallyDrop<TextureRepo<'a>>,

    /// UI Texture repo
    ui_tex_repo: ManuallyDrop<TextureRepo<'a>>,

    /// Buffers used for drawing
    draw_buffers: ManuallyDrop<DrawBuffers<'a, UvPoint>>,

    /// Buffers used for drawing the UI
    ui_draw_buffers: ManuallyDrop<DrawBuffers<'a, UiPoint>>,

    /// View projection matrix
    pub(crate) vp_matrix: Mat4,

    pub(crate) pixels_per_point: f32,
}

impl<'a, M: 'static + MinBspFeatures<VulkanSystem>> RenderingContext<'a, M> {
    /// Create a new RenderingContext for the given window.
    pub fn new(window: &Window, ui: &mut UiState, map: M) -> Result<Self> {
        let map = Arc::new(RwLock::new(map));
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

        let (mut queue_negotiator, surface) = {
            let dq: DrawQueue = DrawQueue { surface };

            let qn = QueueNegotiator::find(&adapter, &[&dq, &TexLoadQueue])
                .context("Error creating draw queue negotiator")?;

            (qn, dq.surface)
        };

        // Device & Queue groups
        let (device_lock, mut queue_groups) = {
            let (df, dqs) = queue_negotiator
                .family_spec::<DrawQueue>(&adapter.queue_families, 1)
                .ok_or(EnvironmentError::NoSuitableFamilies)?;
            let (tf, tqs) = queue_negotiator
                .family_spec::<TexLoadQueue>(&adapter.queue_families, 2)
                .ok_or(EnvironmentError::NoSuitableFamilies)?;

            let gpu = unsafe {
                adapter
                    .physical_device
                    .open(
                        &[(df, dqs.as_slice()), (tf, tqs.as_slice())],
                        hal::Features::empty(),
                    )
                    .context("Error opening logical device")?
            };

            (Arc::new(RwLock::new(gpu.device)), gpu.queue_groups)
        };

        let mut device = device_lock
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;

        // Figure out what our swapchain will look like
        let swapchain_properties = SwapchainProperties::find_best(&adapter, &surface)
            .context("Error getting properties for swapchain")?;

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

        // Vertex and index buffers
        let draw_buffers =
            DrawBuffers::new(&mut device, &adapter).context("Error creating 3D draw buffers")?;

        // UI Vertex and index buffers
        let ui_draw_buffers =
            DrawBuffers::new(&mut device, &adapter).context("Error creating UI draw buffers")?;

        //  We have to unlock device for creating texture repos
        drop(device);

        // Texture repos
        debug!("Creating 3D Texture Repo");
        let tex_repo = TextureRepo::new(
            device_lock.clone(),
            queue_negotiator
                .family::<TexLoadQueue>()
                .ok_or(EnvironmentError::NoQueues)?,
            queue_negotiator
                .get_queue::<TexLoadQueue>(&mut queue_groups)
                .ok_or(EnvironmentError::NoQueues)
                .context("Error getting 3D texture loader queue")?,
            &adapter,
            TextureLoadConfig {
                resolver: FsResolver::new(std::path::Path::new("."), map.clone()),
                filter: hal::image::Filter::Linear,
                wrap_mode: hal::image::WrapMode::Tile,
            },
        )
        .context("Error creating 3D Texture repo")?; // TODO

        debug!("Creating UI Texture Repo");
        let ui_tex_repo = TextureRepo::new(
            device_lock.clone(),
            queue_negotiator
                .family::<TexLoadQueue>()
                .ok_or(EnvironmentError::NoQueues)?,
            queue_negotiator
                .get_queue::<TexLoadQueue>(&mut queue_groups)
                .ok_or(EnvironmentError::NoQueues)
                .context("Error getting UI texture loader queue")?,
            &adapter,
            TextureLoadConfig {
                resolver: UiTextures::new(ui.ctx().clone()),
                filter: hal::image::Filter::Linear,
                wrap_mode: hal::image::WrapMode::Clamp,
            },
        )
        .context("Error creating UI texture repo")?; // TODO

        let mut device = device_lock.write().map_err(|_| LockPoisoned::Device)?;

        let ds_layout_lock = tex_repo.get_ds_layout()?;
        let ui_ds_layout_lock = ui_tex_repo.get_ds_layout()?;

        // Graphics pipeline
        let pipeline = CompletePipeline::new(
            &mut device,
            swapchain_properties.extent,
            &swapchain_properties,
            once(&*ds_layout_lock),
        )?;

        // UI pipeline
        let ui_pipeline = UiPipeline::new(
            &mut device,
            swapchain_properties.extent,
            &swapchain_properties,
            once(&*ui_ds_layout_lock),
        )?;

        // Swapchain and associated resources
        let target_chain = TargetChain::new(
            &mut device,
            &adapter,
            surface,
            &pipeline,
            &ui_pipeline,
            &mut cmd_pool,
            swapchain_properties,
        )
        .context("Error creating target chain")?;

        drop(device);
        drop(ds_layout_lock);
        drop(ui_ds_layout_lock);

        Ok(RenderingContext {
            map,
            instance: ManuallyDrop::new(instance),

            device: device_lock,
            adapter,

            queue: queue_negotiator
                .get_queue::<DrawQueue>(&mut queue_groups)
                .ok_or(EnvironmentError::NoQueues)
                .context("Error getting draw queue")?,

            target_chain: ManuallyDrop::new(target_chain),
            cmd_pool: ManuallyDrop::new(cmd_pool),

            pipeline: ManuallyDrop::new(pipeline),
            ui_pipeline: ManuallyDrop::new(ui_pipeline),

            tex_repo: ManuallyDrop::new(tex_repo),
            ui_tex_repo: ManuallyDrop::new(ui_tex_repo),

            draw_buffers: ManuallyDrop::new(draw_buffers),
            ui_draw_buffers: ManuallyDrop::new(ui_draw_buffers),

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

        use core::ptr::read;

        // Graphics pipeline
        // TODO: Recycle
        let ds_layout_handle = self.tex_repo.get_ds_layout()?;
        let ui_ds_layout_handle = self.tex_repo.get_ds_layout()?;

        ManuallyDrop::into_inner(read(&self.pipeline)).deactivate(&mut device);
        self.pipeline = ManuallyDrop::new({
            CompletePipeline::new(
                &mut device,
                properties.extent,
                &properties,
                once(&*ds_layout_handle),
            )
            .context("Error creating 3D Pipeline")?
        });

        // 2D Graphics pipeline
        // TODO: Recycle
        ManuallyDrop::into_inner(read(&self.ui_pipeline)).deactivate(&mut device);
        self.ui_pipeline = ManuallyDrop::new({
            let mut descriptor_set_layouts: ArrayVec<[_; 1]> = ArrayVec::new();
            descriptor_set_layouts.push(&*ui_ds_layout_handle);

            UiPipeline::new(
                &mut device,
                properties.extent,
                &properties,
                once(&*ui_ds_layout_handle),
            )
            .context("Error creating UI Pipeline")?
        });

        self.target_chain = ManuallyDrop::new(
            TargetChain::new(
                &mut device,
                &self.adapter,
                surface,
                &self.pipeline,
                &self.ui_pipeline,
                &mut self.cmd_pool,
                properties,
            )
            .context("Error creating target chain")?,
        );
        Ok(())
    }

    /// Draw all vertices in the buffer
    pub fn draw_vertices(&mut self, ui: &mut UiState, faces: &[u32]) -> Result<()> {
        let mut device = self
            .device
            .write()
            .map_err(|_| LockPoisoned::Device)
            .context("Error getting device lock")?;
        let mut queue = self
            .queue
            .write()
            .map_err(|_| LockPoisoned::Map)
            .context("Error getting map lock")?;

        // Ensure UI texture(s) are loaded
        ensure_textures_ui(&mut self.ui_tex_repo, ui)?;

        // Get any textures that just finished loading
        self.ui_tex_repo.process_responses();
        self.tex_repo.process_responses();

        // 3D Pass
        let (cmd_buffer, img) = self
            .target_chain
            .prep_next_target(
                &mut device,
                &mut self.draw_buffers,
                &self.pipeline,
                &self.vp_matrix,
            )
            .context("Error preparing next target")?;

        do_render(
            cmd_buffer,
            &mut self.draw_buffers,
            &mut self.tex_repo,
            &self.pipeline.pipeline_layout,
            &*self
                .map
                .read()
                .map_err(|_| LockPoisoned::Map)
                .context("Error getting map read lock")?,
            faces,
        )?;

        // 2D Pass
        let cmd_buffer = self
            .target_chain
            .target_2d_pass(&mut self.ui_draw_buffers, &img, &self.ui_pipeline)
            .context("Error switching to 2D pass")?;

        do_render_ui(
            cmd_buffer,
            &self.ui_pipeline.pipeline_layout,
            &mut self.ui_draw_buffers,
            &mut self.ui_tex_repo,
            ui,
        )?;

        // Update our buffers before we actually start drawing
        self.draw_buffers
            .vertex_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool)?;

        self.draw_buffers
            .index_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool)?;

        self.ui_draw_buffers
            .vertex_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool)?;

        self.ui_draw_buffers
            .index_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool)?;

        // Send commands off to GPU
        self.target_chain
            .finish_and_submit_target(img, &mut queue)
            .context("Error finishing and submitting target")?;

        Ok(())
    }
}

impl<'a, M: MinBspFeatures<VulkanSystem>> core::ops::Drop for RenderingContext<'a, M> {
    fn drop(&mut self) {
        {
            self.device.write().unwrap().wait_idle().unwrap();
        }

        unsafe {
            use core::ptr::read;

            ManuallyDrop::into_inner(read(&self.tex_repo)).deactivate(&mut self.device);
            ManuallyDrop::into_inner(read(&self.ui_tex_repo)).deactivate(&mut self.device);

            let mut device = self.device.write().unwrap();

            ManuallyDrop::into_inner(read(&self.draw_buffers)).deactivate(&mut device);
            ManuallyDrop::into_inner(read(&self.ui_draw_buffers)).deactivate(&mut device);

            ManuallyDrop::into_inner(read(&self.target_chain)).deactivate(
                &mut self.instance,
                &mut device,
                &mut self.cmd_pool,
            );

            device.destroy_command_pool(ManuallyDrop::into_inner(read(&self.cmd_pool)));

            ManuallyDrop::into_inner(read(&self.pipeline)).deactivate(&mut device);
            ManuallyDrop::into_inner(read(&self.ui_pipeline)).deactivate(&mut device);
        }
    }
}
