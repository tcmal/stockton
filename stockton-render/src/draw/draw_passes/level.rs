//! Minimal code for drawing any level, based on traits from stockton-levels

use super::{DrawPass, IntoDrawPass};
use crate::{
    draw::{queue_negotiator::QueueNegotiator, target::SwapchainProperties, texture::TextureRepo},
    error::EnvironmentError,
    types::*,
};
use stockton_levels::features::MinRenderFeatures;
use stockton_types::*;

use std::{
    array::IntoIter,
    iter::{empty, once},
    marker::PhantomData,
    mem::{size_of, ManuallyDrop},
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};

/// The Vertexes that go to the shader
#[derive(Debug, Clone, Copy)]
struct UvPoint(pub Vector3, pub i32, pub Vector2);

/// Draw a level
pub struct LevelDrawPass<M: MinRenderFeatures> {
    pipeline: CompletePipeline,
    repo: TextureRepo,
    _d: PhantomData<M>,
}

impl<M: MinRenderFeatures> DrawPass for LevelDrawPass<M> {
    fn queue_draw(
        &self,
        _input: &Session,
        _cmd_buffer: &mut crate::types::CommandBufferT,
    ) -> anyhow::Result<()> {
        todo!()
        // // Get visible faces
        // // let faces = get_visible_faces(
        // //     pos,
        // //     &*self
        // //         .context
        // //         .map
        // //         .read()
        // //         .map_err(|_| LockPoisoned::Map)
        // //         .context("Error getting read lock on map")?,
        // // );
        // let faces: Vec<u32> = {
        //     let map = &*self
        //         .context
        //         .map
        //         .read()
        //         .map_err(|_| LockPoisoned::Map)
        //         .context("Error getting read lock on map")?;

        //     map.iter_faces().map(|x| x.index(map)).collect()
        // };

        // // Iterate over faces, copying them in and drawing groups that use the same texture chunk all at once.
        // let mut current_chunk = file
        //     .get_face(0)
        //     .ok_or(LevelError::BadReference)?
        //     .texture_idx(file) as usize
        //     / 8;
        // let mut chunk_start = 0;

        // let mut curr_vert_idx: usize = 0;
        // let mut curr_idx_idx: usize = 0;

        // for face in faces.iter().map(|idx| file.get_face(*idx)) {
        //     if let Some(face) = face {
        //         if current_chunk != face.texture_idx(file) as usize / 8 {
        //             // Last index was last of group, so draw it all if textures are loaded.
        //             draw_or_queue(
        //                 current_chunk,
        //                 self.tex_repo,
        //                 cmd_buffer,
        //                 self.pipeline.pipeline_layout,
        //                 chunk_start as u32,
        //                 curr_idx_idx as u32,
        //             )?;

        //             // Next group of same-chunked faces starts here.
        //             chunk_start = curr_idx_idx;
        //             current_chunk = face.texture_idx(file) as usize / 8;
        //         }

        //         match face.geometry(file) {
        //             Geometry::Vertices(v1, v2, v3) => {
        //                 for v in [v1, v2, v3] {
        //                     let uvp =
        //                         UvPoint(v.position, face.texture_idx(file).try_into()?, v.tex);

        //                     draw_buffers.vertex_buffer[curr_vert_idx] = uvp;
        //                     curr_vert_idx += 1;
        //                 }
        //                 draw_buffers.index_buffer[curr_idx_idx] = (
        //                     curr_vert_idx as u16 - 2,
        //                     curr_vert_idx as u16 - 1,
        //                     curr_vert_idx as u16,
        //                 );
        //                 curr_idx_idx += 1;
        //             }
        //         }

        //         if curr_vert_idx >= INITIAL_VERT_SIZE.try_into()?
        //             || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into()?
        //         {
        //             println!("out of vertex buffer space!");
        //             break;
        //         }
        //     } else {
        //         anyhow::bail!(LevelError::BadReference);
        //     }
        // }

        // // Draw the final group of chunks
        // draw_or_queue(
        //     current_chunk,
        //     self.tex_repo,
        //     cmd_buffer,
        //     self.pipeline.pipeline_layout,
        //     chunk_start as u32,
        //     curr_idx_idx as u32,
        // )?;

        // Ok(())
    }

    fn find_aux_queues<'a>(
        _adapter: &'a Adapter,
        _queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>> {
        todo!()
        // queue_negotiator.find(TexLoadQueue)
    }
}

impl<M: MinRenderFeatures> IntoDrawPass<LevelDrawPass<M>> for () {
    fn init(
        self,
        _device_lock: Arc<RwLock<DeviceT>>,
        _queue_negotiator: &mut QueueNegotiator,
        _swapchain_properties: &SwapchainProperties,
    ) -> Result<LevelDrawPass<M>> {
        todo!()
        // let repo = TextureRepo::new(
        //     device_lock.clone(),
        //     queue_negotiator
        //         .family()
        //         .ok_or(EnvironmentError::NoQueues)?,
        // );
        // let pipeline = {
        //     let device = device_lock.write().or(Err(LockPoisoned::Device))?;
        //     CompletePipeline::new(
        //         device,
        //         swapchain_properties.extent,
        //         swapchain_properties,
        //         std::iter::empty(),
        //     )?
        // };
        // Ok(LevelDrawPass {
        //     pipeline,
        //     repo,
        //     _d: PhantomData,
        // })
    }
}

/// Entry point name for shaders
const ENTRY_NAME: &str = "main";

/// Source for vertex shader. TODO
const VERTEX_SOURCE: &str = include_str!("../data/stockton.vert");

/// Source for fragment shader. TODO
const FRAGMENT_SOURCE: &str = include_str!("../data/stockton.frag");

/// A complete graphics pipeline and associated resources
pub struct CompletePipeline {
    /// Our main render pass
    pub(crate) renderpass: ManuallyDrop<RenderPassT>,

    /// The layout of our main graphics pipeline
    pub(crate) pipeline_layout: ManuallyDrop<PipelineLayoutT>,

    /// Our main graphics pipeline
    pub(crate) pipeline: ManuallyDrop<GraphicsPipelineT>,

    /// The vertex shader module
    pub(crate) vs_module: ManuallyDrop<ShaderModuleT>,

    /// The fragment shader module
    pub(crate) fs_module: ManuallyDrop<ShaderModuleT>,
}

impl CompletePipeline {
    pub fn new<'a, T: Iterator<Item = &'a DescriptorSetLayoutT> + std::fmt::Debug>(
        device: &mut DeviceT,
        extent: hal::image::Extent,
        swapchain_properties: &SwapchainProperties,
        set_layouts: T,
    ) -> Result<Self> {
        use hal::format::Format;
        use hal::pso::*;

        // Renderpass
        let renderpass = {
            use hal::{image::Layout, pass::*};

            let img_attachment = Attachment {
                format: Some(swapchain_properties.format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::new(
                    AttachmentLoadOp::Clear,
                    AttachmentStoreOp::DontCare,
                ),
                layouts: Layout::Undefined..Layout::ColorAttachmentOptimal,
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

            unsafe {
                device.create_render_pass(
                    IntoIter::new([img_attachment, depth_attachment]),
                    once(subpass),
                    empty(),
                )
            }
            .context("Error creating render pass")?
        };

        // Subpass
        let subpass = hal::pass::Subpass {
            index: 0,
            main_pass: &renderpass,
        };

        // Shader modules
        let (vs_module, fs_module) = {
            let mut compiler = shaderc::Compiler::new().ok_or(EnvironmentError::NoShaderC)?;

            let vertex_compile_artifact = compiler
                .compile_into_spirv(
                    VERTEX_SOURCE,
                    shaderc::ShaderKind::Vertex,
                    "vertex.vert",
                    ENTRY_NAME,
                    None,
                )
                .context("Error compiling vertex shader")?;

            let fragment_compile_artifact = compiler
                .compile_into_spirv(
                    FRAGMENT_SOURCE,
                    shaderc::ShaderKind::Fragment,
                    "fragment.frag",
                    ENTRY_NAME,
                    None,
                )
                .context("Error compiling fragment shader")?;

            // Make into shader module
            unsafe {
                (
                    device
                        .create_shader_module(vertex_compile_artifact.as_binary())
                        .context("Error creating vertex shader module")?,
                    device
                        .create_shader_module(fragment_compile_artifact.as_binary())
                        .context("Error creating fragment shader module")?,
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

        // Rasterizer
        let rasterizer = Rasterizer {
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::BACK,
            front_face: FrontFace::CounterClockwise,
            depth_clamping: false,
            depth_bias: None,
            conservative: true,
            line_width: State::Static(1.0),
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
                set_layouts.into_iter(),
                // vp matrix, 4x4 f32
                IntoIter::new([(ShaderStageFlags::VERTEX, 0..64)]),
            )
        }
        .context("Error creating pipeline layout")?;

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
            blend_constants: None,
            depth_bounds: None,
        };

        // Primitive assembler
        let primitive_assembler = PrimitiveAssemblerDesc::Vertex {
            buffers: &[VertexBufferDesc {
                binding: 0,
                stride: (size_of::<f32>() * 6) as u32,
                rate: VertexInputRate::Vertex,
            }],
            attributes: &[
                AttributeDesc {
                    location: 0,
                    binding: 0,
                    element: Element {
                        format: Format::Rgb32Sfloat,
                        offset: 0,
                    },
                },
                AttributeDesc {
                    location: 1,
                    binding: 0,
                    element: Element {
                        format: Format::R32Sint,
                        offset: (size_of::<f32>() * 3) as u32,
                    },
                },
                AttributeDesc {
                    location: 2,
                    binding: 0,
                    element: Element {
                        format: Format::Rg32Sfloat,
                        offset: (size_of::<f32>() * 4) as u32,
                    },
                },
            ],
            input_assembler: InputAssemblerDesc::new(Primitive::TriangleList),
            vertex: vs_entry,
            tessellation: None,
            geometry: None,
        };

        // Pipeline description
        let pipeline_desc = GraphicsPipelineDesc {
            label: Some("3D"),
            rasterizer,
            fragment: Some(fs_entry),
            blender,
            depth_stencil,
            multisampling: None,
            baked_states,
            layout: &layout,
            subpass,
            flags: PipelineCreationFlags::empty(),
            parent: BasePipeline::None,
            primitive_assembler,
        };

        // Pipeline
        let pipeline = unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            .context("Error creating graphics pipeline")?;

        Ok(CompletePipeline {
            renderpass: ManuallyDrop::new(renderpass),
            pipeline_layout: ManuallyDrop::new(layout),
            pipeline: ManuallyDrop::new(pipeline),
            vs_module: ManuallyDrop::new(vs_module),
            fs_module: ManuallyDrop::new(fs_module),
        })
    }

    /// Deactivate vulkan resources. Use before dropping
    pub fn deactivate(self, device: &mut DeviceT) {
        unsafe {
            use core::ptr::read;

            device.destroy_render_pass(ManuallyDrop::into_inner(read(&self.renderpass)));

            device.destroy_shader_module(ManuallyDrop::into_inner(read(&self.vs_module)));
            device.destroy_shader_module(ManuallyDrop::into_inner(read(&self.fs_module)));

            device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));

            device.destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));
        }
    }
}

fn draw_or_queue(
    current_chunk: usize,
    tex_repo: &mut TextureRepo,
    cmd_buffer: &mut CommandBufferT,
    pipeline_layout: &PipelineLayoutT,
    chunk_start: u32,
    curr_idx_idx: u32,
) -> Result<()> {
    if let Some(ds) = tex_repo.attempt_get_descriptor_set(current_chunk) {
        unsafe {
            cmd_buffer.bind_graphics_descriptor_sets(pipeline_layout, 0, once(ds), empty());
            cmd_buffer.draw_indexed(chunk_start * 3..(curr_idx_idx * 3) + 1, 0, 0..1);
        }
    } else {
        tex_repo.queue_load(current_chunk)?
    }

    Ok(())
}
