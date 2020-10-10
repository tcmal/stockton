// Copyright (C) 2019 Oscar Shrimpton

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

//! Deals with all the Vulkan/HAL details.
//! In the end, this takes in a depth-sorted list of faces and a map file and renders them.
//! You'll need something else to actually find/sort the faces though.

use std::{
    borrow::Borrow,
    convert::TryInto,
    mem::{size_of, ManuallyDrop},
    ops::Deref,
};

use arrayvec::ArrayVec;
use hal::{pool::CommandPoolCreateFlags, prelude::*};
use log::debug;
use winit::window::Window;

use stockton_levels::prelude::*;
use stockton_levels::traits::faces::FaceType;
use stockton_types::{Vector2, Vector3};

use super::{
    buffer::ModifiableBuffer,
    camera::WorkingCamera,
    draw_buffers::{DrawBuffers, INITIAL_INDEX_SIZE, INITIAL_VERT_SIZE},
    target::{SwapchainProperties, TargetChain},
    texture::TextureStore,
};
use crate::{error, types::*};

/// Entry point name for shaders
const ENTRY_NAME: &str = "main";

/// Source for vertex shader. TODO
const VERTEX_SOURCE: &str = include_str!("./data/stockton.vert");

/// Source for fragment shader. TODO
const FRAGMENT_SOURCE: &str = include_str!("./data/stockton.frag");

/// Represents a point of a triangle, including UV and texture information.
#[derive(Debug, Clone, Copy)]
pub struct UVPoint(pub Vector3, pub i32, pub Vector2);

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
    target_chain: ManuallyDrop<TargetChain>,

    // Pipeline
    /// Our main render pass
    renderpass: ManuallyDrop<RenderPass>,

    /// The layout of our main graphics pipeline
    pipeline_layout: ManuallyDrop<PipelineLayout>,

    /// Our main graphics pipeline
    pipeline: ManuallyDrop<GraphicsPipeline>,

    // Command pool and buffers
    /// The command pool used for our buffers
    cmd_pool: ManuallyDrop<CommandPool>,

    /// The queue group our buffers belong to
    queue_group: QueueGroup,

    /// Texture store
    texture_store: ManuallyDrop<TextureStore>,

    /// Buffers used for drawing
    draw_buffers: ManuallyDrop<DrawBuffers<'a>>,

    /// Our camera settings
    camera: WorkingCamera,

    /// The vertex shader module
    vs_module: ManuallyDrop<ShaderModule>,

    /// The fragment shader module
    fs_module: ManuallyDrop<ShaderModule>,
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

        // Renderpass
        let renderpass = {
            use hal::{
                image::{Access, Layout},
                memory::Dependencies,
                pass::*,
                pso::PipelineStage,
            };

            let img_attachment = Attachment {
                format: Some(swapchain_properties.format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::new(
                    AttachmentLoadOp::Clear,
                    AttachmentStoreOp::DontCare,
                ),
                layouts: Layout::Undefined..Layout::Present,
            };

            let depth_attachment = Attachment {
                format: Some(swapchain_properties.depth_format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::DontCare),
                stencil_ops: AttachmentOps::new(
                    AttachmentLoadOp::DontCare,
                    AttachmentStoreOp::DontCare,
                ),
                layouts: Layout::Undefined..Layout::DepthStencilAttachmentOptimal,
            };

            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: Some(&(1, Layout::DepthStencilAttachmentOptimal)),
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let in_dependency = SubpassDependency {
                flags: Dependencies::empty(),
                passes: None..Some(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..(PipelineStage::COLOR_ATTACHMENT_OUTPUT
                        | PipelineStage::EARLY_FRAGMENT_TESTS),
                accesses: Access::empty()
                    ..(Access::COLOR_ATTACHMENT_READ
                        | Access::COLOR_ATTACHMENT_WRITE
                        | Access::DEPTH_STENCIL_ATTACHMENT_READ
                        | Access::DEPTH_STENCIL_ATTACHMENT_WRITE),
            };

            let out_dependency = SubpassDependency {
                flags: Dependencies::empty(),
                passes: Some(0)..None,
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT | PipelineStage::EARLY_FRAGMENT_TESTS
                    ..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
                accesses: (Access::COLOR_ATTACHMENT_READ
                    | Access::COLOR_ATTACHMENT_WRITE
                    | Access::DEPTH_STENCIL_ATTACHMENT_READ
                    | Access::DEPTH_STENCIL_ATTACHMENT_WRITE)
                    ..Access::empty(),
            };

            unsafe {
                device.create_render_pass(
                    &[img_attachment, depth_attachment],
                    &[subpass],
                    &[in_dependency, out_dependency],
                )
            }
            .map_err(|_| error::CreationError::OutOfMemoryError)?
        };

        // Subpass
        let subpass = hal::pass::Subpass {
            index: 0,
            main_pass: &renderpass,
        };

        // Camera
        // TODO: Settings
        let ratio =
            swapchain_properties.extent.width as f32 / swapchain_properties.extent.height as f32;
        let camera = WorkingCamera::defaults(ratio);

        // Vertex and index buffers
        let draw_buffers = DrawBuffers::new(&mut device, &adapter)?;

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
        let (pipeline_layout, pipeline, vs_module, fs_module) = Self::create_pipeline(
            &mut device,
            swapchain_properties.extent,
            &subpass,
            descriptor_set_layouts,
        )?;

        // Swapchain and associated resources
        let target_chain = TargetChain::new(
            &mut device,
            &adapter,
            &mut surface,
            &renderpass,
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

            renderpass: ManuallyDrop::new(renderpass),
            target_chain: ManuallyDrop::new(target_chain),
            cmd_pool: ManuallyDrop::new(cmd_pool),

            pipeline_layout: ManuallyDrop::new(pipeline_layout),
            pipeline: ManuallyDrop::new(pipeline),

            texture_store: ManuallyDrop::new(texture_store),

            draw_buffers: ManuallyDrop::new(draw_buffers),

            vs_module: ManuallyDrop::new(vs_module),
            fs_module: ManuallyDrop::new(fs_module),

            camera,
        })
    }

    /// If this function fails the whole context is probably dead
    /// # Safety
    /// The context must not be used while this is being called
    pub unsafe fn handle_surface_change(&mut self) -> Result<(), error::CreationError> {
        self.device.wait_idle().unwrap();

        let properties = SwapchainProperties::find_best(&self.adapter, &self.surface)
            .map_err(|_| error::CreationError::BadSurface)?;

        // Camera settings (aspect ratio)
        self.camera
            .update_aspect_ratio(properties.extent.width as f32 / properties.extent.height as f32);

        use core::ptr::read;

        // Graphics pipeline
        self.device
            .destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));

        self.device
            .destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));

        self.device
            .destroy_shader_module(ManuallyDrop::into_inner(read(&self.vs_module)));
        self.device
            .destroy_shader_module(ManuallyDrop::into_inner(read(&self.fs_module)));

        let (pipeline_layout, pipeline, vs_module, fs_module) = {
            let mut descriptor_set_layouts: ArrayVec<[_; 2]> = ArrayVec::new();
            descriptor_set_layouts.push(self.texture_store.descriptor_set_layout.deref());

            let subpass = hal::pass::Subpass {
                index: 0,
                main_pass: &(*self.renderpass),
            };

            Self::create_pipeline(
                &mut self.device,
                properties.extent,
                &subpass,
                descriptor_set_layouts,
            )?
        };

        self.pipeline_layout = ManuallyDrop::new(pipeline_layout);
        self.pipeline = ManuallyDrop::new(pipeline);

        self.vs_module = ManuallyDrop::new(vs_module);
        self.fs_module = ManuallyDrop::new(fs_module);

        let old_swapchain = ManuallyDrop::into_inner(read(&self.target_chain))
            .deactivate_with_recyling(&mut self.device, &mut self.cmd_pool);
        self.target_chain = ManuallyDrop::new(
            TargetChain::new(
                &mut self.device,
                &self.adapter,
                &mut self.surface,
                &self.renderpass,
                &mut self.cmd_pool,
                properties,
                Some(old_swapchain),
            )
            .map_err(error::CreationError::TargetChainCreationError)?,
        );

        Ok(())
    }

    #[allow(clippy::type_complexity)]
    fn create_pipeline<T>(
        device: &mut Device,
        extent: hal::image::Extent,
        subpass: &hal::pass::Subpass<back::Backend>,
        set_layouts: T,
    ) -> Result<(PipelineLayout, GraphicsPipeline, ShaderModule, ShaderModule), error::CreationError>
    where
        T: IntoIterator,
        T::Item: Borrow<DescriptorSetLayout>,
    {
        use hal::format::Format;
        use hal::pso::*;

        // Shader modules
        let (vs_module, fs_module) = {
            let mut compiler = shaderc::Compiler::new().ok_or(error::CreationError::NoShaderC)?;

            let vertex_compile_artifact = compiler
                .compile_into_spirv(
                    VERTEX_SOURCE,
                    shaderc::ShaderKind::Vertex,
                    "vertex.vert",
                    ENTRY_NAME,
                    None,
                )
                .map_err(error::CreationError::ShaderCError)?;

            let fragment_compile_artifact = compiler
                .compile_into_spirv(
                    FRAGMENT_SOURCE,
                    shaderc::ShaderKind::Fragment,
                    "fragment.frag",
                    ENTRY_NAME,
                    None,
                )
                .map_err(error::CreationError::ShaderCError)?;

            // Make into shader module
            unsafe {
                (
                    device
                        .create_shader_module(vertex_compile_artifact.as_binary())
                        .map_err(error::CreationError::ShaderModuleFailed)?,
                    device
                        .create_shader_module(fragment_compile_artifact.as_binary())
                        .map_err(error::CreationError::ShaderModuleFailed)?,
                )
            }
        };

        // Shader entry points (ShaderStage)
        let (vs_entry, fs_entry) = (
            EntryPoint::<back::Backend> {
                entry: ENTRY_NAME,
                module: &vs_module,
                specialization: Specialization::default(),
            },
            EntryPoint::<back::Backend> {
                entry: ENTRY_NAME,
                module: &fs_module,
                specialization: Specialization::default(),
            },
        );

        // Shader set
        let shaders = GraphicsShaderSet {
            vertex: vs_entry,
            fragment: Some(fs_entry),
            hull: None,
            domain: None,
            geometry: None,
        };

        // Vertex buffers
        let vertex_buffers: Vec<VertexBufferDesc> = vec![VertexBufferDesc {
            binding: 0,
            stride: (size_of::<f32>() * 6) as u32,
            rate: VertexInputRate::Vertex,
        }];

        let attributes: Vec<AttributeDesc> = pipeline_vb_attributes!(0,
            size_of::<f32>() * 3; Rgb32Sfloat,
            size_of::<u32>(); R32Sint,
            size_of::<f32>() * 2; Rg32Sfloat
        );

        // Rasterizer
        let rasterizer = Rasterizer {
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::BACK,
            front_face: FrontFace::CounterClockwise,
            depth_clamping: false,
            depth_bias: None,
            conservative: true,
            line_width: hal::pso::State::Static(1.0),
        };

        // Depth stencil
        let depth_stencil = DepthStencilDesc {
            depth: Some(DepthTest {
                fun: Comparison::Less,
                write: true,
            }),
            depth_bounds: false,
            stencil: None,
        };

        // Pipeline layout
        let layout = unsafe {
            device.create_pipeline_layout(
                set_layouts,
                // vp matrix, 4x4 f32
                &[(ShaderStageFlags::VERTEX, 0..64)],
            )
        }
        .map_err(|_| error::CreationError::OutOfMemoryError)?;

        // Colour blending
        let blender = {
            let blend_state = BlendState {
                color: BlendOp::Add {
                    src: Factor::One,
                    dst: Factor::Zero,
                },
                alpha: BlendOp::Add {
                    src: Factor::One,
                    dst: Factor::Zero,
                },
            };

            BlendDesc {
                logic_op: Some(LogicOp::Copy),
                targets: vec![ColorBlendDesc {
                    mask: ColorMask::ALL,
                    blend: Some(blend_state),
                }],
            }
        };

        // Baked states
        let baked_states = BakedStates {
            viewport: Some(Viewport {
                rect: extent.rect(),
                depth: (0.0..1.0),
            }),
            scissor: Some(extent.rect()),
            blend_color: None,
            depth_bounds: None,
        };

        // Input assembler
        let input_assembler = InputAssemblerDesc::new(Primitive::TriangleList);

        // Pipeline description
        let pipeline_desc = GraphicsPipelineDesc {
            shaders,
            rasterizer,
            vertex_buffers,
            blender,
            depth_stencil,
            multisampling: None,
            baked_states,
            layout: &layout,
            subpass: *subpass,
            flags: PipelineCreationFlags::empty(),
            parent: BasePipeline::None,
            input_assembler,
            attributes,
        };

        // Pipeline
        let pipeline = unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            .map_err(error::CreationError::PipelineError)?;

        Ok((layout, pipeline, vs_module, fs_module))
    }

    /// Draw all vertices in the buffer
    pub fn draw_vertices<M: MinBSPFeatures<VulkanSystem>>(
        &mut self,
        file: &M,
        faces: &[u32],
    ) -> Result<(), &'static str> {
        // Prepare command buffer
        let cmd_buffer = self.target_chain.prep_next_target(
            &mut self.device,
            &mut self.draw_buffers,
            &self.renderpass,
            &self.pipeline,
            &self.pipeline_layout,
            &mut self.camera,
        )?;

        // Iterate over faces, copying them in and drawing groups that use the same texture chunk all at once.
        let mut current_chunk = file.get_face(0).texture_idx as usize / 8;
        let mut chunk_start = 0;

        let mut curr_vert_idx: usize = 0;
        let mut curr_idx_idx: usize = 0;

        for face in faces.iter().map(|idx| file.get_face(*idx)) {
            if current_chunk != face.texture_idx as usize / 8 {
                // Last index was last of group, so draw it all.
                let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
                descriptor_sets.push(self.texture_store.get_chunk_descriptor_set(current_chunk));
                unsafe {
                    cmd_buffer.bind_graphics_descriptor_sets(
                        &self.pipeline_layout,
                        0,
                        descriptor_sets,
                        &[],
                    );
                    cmd_buffer.draw_indexed(
                        chunk_start as u32 * 3..(curr_idx_idx as u32 * 3) + 1,
                        0,
                        0..1,
                    );
                }

                // Next group of same-chunked faces starts here.
                chunk_start = curr_idx_idx;
                current_chunk = face.texture_idx as usize / 8;
            }

            if face.face_type == FaceType::Polygon || face.face_type == FaceType::Mesh {
                // 2 layers of indirection
                let base = face.vertices_idx.start;

                for idx in face.meshverts_idx.clone().step_by(3) {
                    let start_idx: u16 = curr_vert_idx.try_into().unwrap();

                    for idx2 in idx..idx + 3 {
                        let vert = &file.resolve_meshvert(idx2 as u32, base);
                        let uv = Vector2::new(vert.tex.u[0], vert.tex.v[0]);

                        let uvp = UVPoint(vert.position, face.texture_idx.try_into().unwrap(), uv);
                        self.draw_buffers.vertex_buffer[curr_vert_idx] = uvp;

                        curr_vert_idx += 1;
                    }

                    self.draw_buffers.index_buffer[curr_idx_idx] =
                        (start_idx, start_idx + 1, start_idx + 2);

                    curr_idx_idx += 1;

                    if curr_vert_idx >= INITIAL_VERT_SIZE.try_into().unwrap()
                        || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into().unwrap()
                    {
                        println!("out of vertex buffer space!");
                        break;
                    }
                }
            } else {
                // TODO: Other types of faces
            }

            if curr_vert_idx >= INITIAL_VERT_SIZE.try_into().unwrap()
                || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into().unwrap()
            {
                println!("out of vertex buffer space!");
                break;
            }
        }

        // Draw the final group of chunks
        let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
        descriptor_sets.push(self.texture_store.get_chunk_descriptor_set(current_chunk));
        unsafe {
            cmd_buffer.bind_graphics_descriptor_sets(
                &self.pipeline_layout,
                0,
                descriptor_sets,
                &[],
            );
            cmd_buffer.draw_indexed(
                chunk_start as u32 * 3..(curr_idx_idx as u32 * 3) + 1,
                0,
                0..1,
            );
        }

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

        // Send commands off to GPU
        self.target_chain
            .finish_and_submit_target(&mut self.queue_group.queues[0])?;

        Ok(())
    }

    /// Get current position of camera
    pub fn camera_pos(&self) -> Vector3 {
        self.camera.camera_pos()
    }

    /// Move the camera by `delta` relative to its rotation
    pub fn move_camera_relative(&mut self, delta: Vector3) {
        self.camera.move_camera_relative(delta)
    }

    /// Rotate the camera
    /// `euler` should be euler angles in radians
    pub fn rotate(&mut self, euler: Vector3) {
        self.camera.rotate(euler)
    }
}

impl<'a> core::ops::Drop for RenderingContext<'a> {
    fn drop(&mut self) {
        self.device.wait_idle().unwrap();

        unsafe {
            use core::ptr::read;

            ManuallyDrop::into_inner(read(&self.draw_buffers)).deactivate(&mut self.device);
            ManuallyDrop::into_inner(read(&self.texture_store)).deactivate(&mut self.device);

            ManuallyDrop::into_inner(read(&self.target_chain))
                .deactivate(&mut self.device, &mut self.cmd_pool);

            self.device
                .destroy_command_pool(ManuallyDrop::into_inner(read(&self.cmd_pool)));
            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(read(&self.renderpass)));

            self.device
                .destroy_shader_module(ManuallyDrop::into_inner(read(&self.vs_module)));
            self.device
                .destroy_shader_module(ManuallyDrop::into_inner(read(&self.fs_module)));

            self.device
                .destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));

            self.device
                .destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));

            self.instance
                .destroy_surface(ManuallyDrop::into_inner(read(&self.surface)));

            ManuallyDrop::drop(&mut self.device);
        }
    }
}
