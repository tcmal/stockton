//! A complete graphics pipeline

/// Entry point name for shaders
const ENTRY_NAME: &str = "main";

/// Source for vertex shader. TODO
const VERTEX_SOURCE: &str = include_str!("./data/stockton.vert");

/// Source for fragment shader. TODO
const FRAGMENT_SOURCE: &str = include_str!("./data/stockton.frag");

use std::{
    array::IntoIter,
    iter::{empty, once},
    mem::{size_of, ManuallyDrop},
};

use super::target::SwapchainProperties;
use crate::{error::EnvironmentError, types::*};
use anyhow::{Context, Result};

// TODO: Generalise so we can use for UI also
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
