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
//! A complete graphics pipeline

/// Entry point name for shaders
const ENTRY_NAME: &str = "main";

/// Source for vertex shader. TODO
const VERTEX_SOURCE: &str = include_str!("./data/stockton.vert");

/// Source for fragment shader. TODO
const FRAGMENT_SOURCE: &str = include_str!("./data/stockton.frag");

use std::{
    borrow::Borrow,
    mem::{size_of, ManuallyDrop},
};

use hal::prelude::*;

use crate::draw::target::SwapchainProperties;
use crate::error;
use crate::types::*;

/// A complete 2D graphics pipeline and associated resources
pub struct UIPipeline {
    /// Our main render pass
    pub(crate) renderpass: ManuallyDrop<RenderPass>,

    /// The layout of our main graphics pipeline
    pub(crate) pipeline_layout: ManuallyDrop<PipelineLayout>,

    /// Our main graphics pipeline
    pub(crate) pipeline: ManuallyDrop<GraphicsPipeline>,

    /// The vertex shader module
    pub(crate) vs_module: ManuallyDrop<ShaderModule>,

    /// The fragment shader module
    pub(crate) fs_module: ManuallyDrop<ShaderModule>,
}

impl UIPipeline {
    pub fn new<T>(
        device: &mut Device,
        extent: hal::image::Extent,
        swapchain_properties: &SwapchainProperties,
        set_layouts: T,
    ) -> Result<Self, error::CreationError>
    where
        T: IntoIterator + std::fmt::Debug,
        T::Item: Borrow<DescriptorSetLayout>,
    {
        use hal::format::Format;
        use hal::pso::*;

        // Renderpass
        let renderpass = {
            use hal::{
                image::{Access, Layout},
                memory::Dependencies,
                pass::*,
            };

            let img_attachment = Attachment {
                format: Some(swapchain_properties.format),
                samples: 1,
                ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
                stencil_ops: AttachmentOps::new(
                    AttachmentLoadOp::DontCare,
                    AttachmentStoreOp::DontCare,
                ),
                layouts: Layout::ColorAttachmentOptimal..Layout::Present,
            };

            let subpass = SubpassDesc {
                colors: &[(0, Layout::ColorAttachmentOptimal)],
                depth_stencil: None,
                inputs: &[],
                resolves: &[],
                preserves: &[],
            };

            let external_dependency = SubpassDependency {
                flags: Dependencies::empty(),
                passes: None..Some(0),
                stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT
                    ..(PipelineStage::COLOR_ATTACHMENT_OUTPUT
                        | PipelineStage::EARLY_FRAGMENT_TESTS),
                accesses: Access::empty()
                    ..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE),
            };

            unsafe {
                device.create_render_pass(&[img_attachment], &[subpass], &[external_dependency])
            }
            .map_err(|_| error::CreationError::OutOfMemoryError)?
        };

        // Subpass
        let subpass = hal::pass::Subpass {
            index: 0,
            main_pass: &renderpass,
        };

        // Shader modules
        let (vs_module, fs_module) = {
            let mut compiler = shaderc::Compiler::new().ok_or(error::CreationError::NoShaderC)?;

            let vertex_compile_artifact = compiler
                .compile_into_spirv(
                    VERTEX_SOURCE,
                    shaderc::ShaderKind::Vertex,
                    "vertex_ui.vert",
                    ENTRY_NAME,
                    None,
                )
                .map_err(error::CreationError::ShaderCError)?;

            let fragment_compile_artifact = compiler
                .compile_into_spirv(
                    FRAGMENT_SOURCE,
                    shaderc::ShaderKind::Fragment,
                    "fragment_ui.frag",
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
            stride: ((size_of::<f32>() * 4) + (size_of::<u8>() * 4)) as u32,
            rate: VertexInputRate::Vertex,
        }];

        let attributes: Vec<AttributeDesc> = pipeline_vb_attributes!(0,
            size_of::<f32>() * 2; Rg32Sfloat,
            size_of::<f32>() * 2; Rg32Sfloat,
            size_of::<u8>() * 4; Rgba8Uint
        );

        // Rasterizer
        let rasterizer = Rasterizer {
            polygon_mode: PolygonMode::Fill,
            cull_face: Face::NONE,
            front_face: FrontFace::CounterClockwise,
            depth_clamping: false,
            depth_bias: None,
            conservative: true,
            line_width: hal::pso::State::Static(1.0),
        };

        // Depth stencil
        let depth_stencil = DepthStencilDesc {
            depth: None,
            depth_bounds: false,
            stencil: None,
        };

        log::debug!("ui set layouts: {:?}", set_layouts);
        // Pipeline layout
        let layout = unsafe {
            device.create_pipeline_layout(set_layouts, &[(ShaderStageFlags::VERTEX, 0..8)])
        }
        .map_err(|_| error::CreationError::OutOfMemoryError)?;

        // Colour blending
        let blender = {
            let blend_state = BlendState {
                color: BlendOp::Add {
                    src: Factor::SrcAlpha,
                    dst: Factor::OneMinusSrcAlpha,
                },
                alpha: BlendOp::Add {
                    src: Factor::OneMinusSrcAlpha,
                    dst: Factor::Zero,
                },
            };

            BlendDesc {
                logic_op: None,
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
            subpass,
            flags: PipelineCreationFlags::empty(),
            parent: BasePipeline::None,
            input_assembler,
            attributes,
        };

        // Pipeline
        let pipeline = unsafe { device.create_graphics_pipeline(&pipeline_desc, None) }
            .map_err(error::CreationError::PipelineError)?;

        Ok(UIPipeline {
            renderpass: ManuallyDrop::new(renderpass),
            pipeline_layout: ManuallyDrop::new(layout),
            pipeline: ManuallyDrop::new(pipeline),
            vs_module: ManuallyDrop::new(vs_module),
            fs_module: ManuallyDrop::new(fs_module),
        })
    }

    /// Deactivate vulkan resources. Use before dropping
    pub fn deactivate(self, device: &mut Device) {
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
