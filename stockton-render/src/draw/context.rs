//! Deals with all the Vulkan/HAL details.
//! In the end, this takes in a depth-sorted list of faces and a map file and renders them.
//! You'll need something else to actually find/sort the faces though.

use std::{
    iter::once,
    mem::ManuallyDrop,
    sync::{Arc, RwLock},
};

use arrayvec::ArrayVec;
use hal::{memory::SparseFlags, pool::CommandPoolCreateFlags};
use log::debug;
use na::Mat4;
use rendy_memory::DynamicConfig;
use winit::window::Window;

use super::{
    buffer::ModifiableBuffer,
    draw_buffers::{DrawBuffers, UvPoint},
    pipeline::CompletePipeline,
    queue_negotiator::QueueNegotiator,
    render::do_render,
    target::{SwapchainProperties, TargetChain},
    texture::{resolver::BasicFsResolver, TextureRepo},
    ui::{
        do_render as do_render_ui, ensure_textures as ensure_textures_ui, UiPipeline, UiPoint,
        UiTextures,
    },
    utils::find_memory_type_id,
};
use crate::{error, types::*, window::UiState};
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

    /// Memory allocator used for any sort of textures / maps
    /// Guaranteed suitable for 2D RGBA images with `Optimal` tiling and `Usage::Sampled`
    texture_allocator: ManuallyDrop<DynamicAllocator>,

    /// View projection matrix
    pub(crate) vp_matrix: Mat4,

    pub(crate) pixels_per_point: f32,
}

impl<'a, M: 'static + MinBspFeatures<VulkanSystem>> RenderingContext<'a, M> {
    /// Create a new RenderingContext for the given window.
    pub fn new(window: &Window, map: M) -> Result<Self, error::CreationError> {
        let map = Arc::new(RwLock::new(map));
        // Create surface
        let (instance, surface, mut adapters) = unsafe {
            let instance = back::Instance::create("stockton", 1)
                .map_err(|_| error::CreationError::WindowError)?;
            let surface = instance
                .create_surface(window)
                .map_err(|_| error::CreationError::WindowError)?;
            let adapters = instance.enumerate_adapters();

            (instance, surface, adapters)
        };

        // TODO: Properly figure out which adapter to use
        let adapter = adapters.remove(0);

        let mut draw_queue_negotiator = QueueNegotiator::find(&adapter, |family| {
            surface.supports_queue_family(family) && family.queue_type().supports_graphics()
        })
        .unwrap();

        let mut tex_queue_negotiator =
            QueueNegotiator::find(&adapter, TextureRepo::queue_family_filter).unwrap();
        // Device & Queue group
        let (device_lock, mut queue_groups) = {
            debug!(
                "Using draw queue family {:?}",
                draw_queue_negotiator.family_id()
            );
            debug!(
                "Using tex queue family {:?}",
                tex_queue_negotiator.family_id()
            );

            let gpu = unsafe {
                adapter
                    .physical_device
                    .open(
                        &[
                            (draw_queue_negotiator.family(&adapter), &[1.0]),
                            (tex_queue_negotiator.family(&adapter), &[1.0]),
                        ],
                        hal::Features::empty(),
                    )
                    .unwrap()
            };

            (Arc::new(RwLock::new(gpu.device)), gpu.queue_groups)
        };

        let mut device = device_lock.write().unwrap();

        let device_props = adapter.physical_device.properties();

        // Figure out what our swapchain will look like
        let swapchain_properties = SwapchainProperties::find_best(&adapter, &surface)
            .map_err(|_| error::CreationError::BadSurface)?;

        debug!(
            "Detected following swapchain properties: {:?}",
            swapchain_properties
        );

        // Command pool
        let mut cmd_pool = unsafe {
            device.create_command_pool(
                draw_queue_negotiator.family_id(),
                CommandPoolCreateFlags::RESET_INDIVIDUAL,
            )
        }
        .map_err(|_| error::CreationError::OutOfMemoryError)?;

        // Vertex and index buffers
        let draw_buffers = DrawBuffers::new(&mut device, &adapter)?;

        // UI Vertex and index buffers
        let ui_draw_buffers = DrawBuffers::new(&mut device, &adapter)?;

        // Memory allocators
        let texture_allocator = unsafe {
            use hal::{
                format::Format,
                image::{Kind, Tiling, Usage, ViewCapabilities},
                memory::Properties,
            };

            // We create an empty image with the same format as used for textures
            // this is to get the type_mask required, which will stay the same for
            // all colour images of the same tiling. (certain memory flags excluded).

            // Size and alignment don't necessarily stay the same, so we're forced to
            // guess at the alignment for our allocator.

            // TODO: Way to tune these options

            let img = device
                .create_image(
                    Kind::D2(16, 16, 1, 1),
                    1,
                    Format::Rgba8Srgb,
                    Tiling::Optimal,
                    Usage::SAMPLED,
                    SparseFlags::empty(),
                    ViewCapabilities::empty(),
                )
                .map_err(|_| error::CreationError::OutOfMemoryError)?;

            let type_mask = device.get_image_requirements(&img).type_mask;

            device.destroy_image(img);

            let props = Properties::DEVICE_LOCAL;

            DynamicAllocator::new(
                find_memory_type_id(&adapter, type_mask, props)
                    .ok_or(error::CreationError::OutOfMemoryError)?,
                props,
                DynamicConfig {
                    block_size_granularity: 4 * 32 * 32, // 32x32 image
                    max_chunk_size: u64::pow(2, 63),
                    min_device_allocation: 4 * 32 * 32,
                },
                device_props.limits.non_coherent_atom_size as u64,
            )
        };

        drop(device);

        // Texture repos
        debug!("Creating 3D Texture Repo");
        let tex_repo = TextureRepo::new(
            device_lock.clone(),
            tex_queue_negotiator.family_id(),
            tex_queue_negotiator.get_queue(&mut queue_groups).unwrap(),
            &adapter,
            map.clone(),
            BasicFsResolver::new(std::path::Path::new(".")),
        )
        .unwrap(); // TODO

        debug!("Creating UI Texture Repo");
        let ui_tex_repo = TextureRepo::new(
            device_lock.clone(),
            tex_queue_negotiator.family_id(),
            tex_queue_negotiator.get_queue(&mut queue_groups).unwrap(),
            &adapter,
            Arc::new(RwLock::new(UiTextures)),
            BasicFsResolver::new(std::path::Path::new(".")),
        )
        .unwrap(); // TODO

        let mut device = device_lock.write().unwrap();

        let ds_layout_lock = tex_repo.get_ds_layout();
        let ui_ds_layout_lock = ui_tex_repo.get_ds_layout();

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
        .map_err(error::CreationError::TargetChainCreationError)?;

        drop(device);
        drop(ds_layout_lock);
        drop(ui_ds_layout_lock);

        Ok(RenderingContext {
            map,
            instance: ManuallyDrop::new(instance),

            device: device_lock,
            adapter,

            queue: draw_queue_negotiator.get_queue(&mut queue_groups).unwrap(),

            target_chain: ManuallyDrop::new(target_chain),
            cmd_pool: ManuallyDrop::new(cmd_pool),

            pipeline: ManuallyDrop::new(pipeline),
            ui_pipeline: ManuallyDrop::new(ui_pipeline),

            tex_repo: ManuallyDrop::new(tex_repo),
            ui_tex_repo: ManuallyDrop::new(ui_tex_repo),

            draw_buffers: ManuallyDrop::new(draw_buffers),
            ui_draw_buffers: ManuallyDrop::new(ui_draw_buffers),

            texture_allocator: ManuallyDrop::new(texture_allocator),

            vp_matrix: Mat4::identity(),

            pixels_per_point: window.scale_factor() as f32,
        })
    }

    /// If this function fails the whole context is probably dead
    /// # Safety
    /// The context must not be used while this is being called
    pub unsafe fn handle_surface_change(&mut self) -> Result<(), error::CreationError> {
        let mut device = self.device.write().unwrap();

        device.wait_idle().unwrap();

        let surface = ManuallyDrop::into_inner(read(&self.target_chain))
            .deactivate_with_recyling(&mut device, &mut self.cmd_pool);

        let properties = SwapchainProperties::find_best(&self.adapter, &surface)
            .map_err(|_| error::CreationError::BadSurface)?;

        use core::ptr::read;

        // Graphics pipeline
        // TODO: Recycle
        let ds_layout_handle = self.tex_repo.get_ds_layout();
        let ui_ds_layout_handle = self.tex_repo.get_ds_layout();

        ManuallyDrop::into_inner(read(&self.pipeline)).deactivate(&mut device);
        self.pipeline = ManuallyDrop::new({
            CompletePipeline::new(
                &mut device,
                properties.extent,
                &properties,
                once(&*ds_layout_handle),
            )?
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
            )?
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
            .map_err(error::CreationError::TargetChainCreationError)?,
        );
        Ok(())
    }

    /// Draw all vertices in the buffer
    pub fn draw_vertices(&mut self, ui: &mut UiState, faces: &[u32]) -> Result<(), &'static str> {
        let mut device = self.device.write().unwrap();
        let mut queue = self.queue.write().unwrap();

        // Ensure UI texture(s) are loaded
        ensure_textures_ui(
            &mut self.ui_tex_repo,
            ui,
            &mut device,
            &mut self.adapter,
            &mut self.texture_allocator,
            &mut queue,
            &mut self.cmd_pool,
        );

        // Get any textures that just finished loading
        self.tex_repo.process_responses();

        // 3D Pass
        let (cmd_buffer, img) = self.target_chain.prep_next_target(
            &mut device,
            &mut self.draw_buffers,
            &self.pipeline,
            &self.vp_matrix,
        )?;
        do_render(
            cmd_buffer,
            &mut self.draw_buffers,
            &mut self.tex_repo,
            &self.pipeline.pipeline_layout,
            &*self.map.read().unwrap(),
            faces,
        );

        // 2D Pass
        let cmd_buffer =
            self.target_chain
                .target_2d_pass(&mut self.ui_draw_buffers, &img, &self.ui_pipeline)?;
        do_render_ui(
            cmd_buffer,
            &self.ui_pipeline.pipeline_layout,
            &mut self.ui_draw_buffers,
            &mut self.ui_tex_repo,
            ui,
        );

        // Update our buffers before we actually start drawing
        self.draw_buffers
            .vertex_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool);

        self.draw_buffers
            .index_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool);

        self.ui_draw_buffers
            .vertex_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool);

        self.ui_draw_buffers
            .index_buffer
            .commit(&device, &mut queue, &mut self.cmd_pool);

        // Send commands off to GPU
        self.target_chain
            .finish_and_submit_target(img, &mut queue)?;

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

            ManuallyDrop::into_inner(read(&self.texture_allocator)).dispose();

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
