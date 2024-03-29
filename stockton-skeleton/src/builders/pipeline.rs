use super::{renderpass::RenderpassSpec, shader::ShaderDesc};
use crate::{error::EnvironmentError, types::*, utils::get_pixel_size};

use std::{mem::ManuallyDrop, ops::Range};

use anyhow::{Context, Result};
use hal::{
    format::Format,
    pso::{
        AttributeDesc, BakedStates, BasePipeline, BlendDesc, BufferIndex, DepthStencilDesc,
        ElemStride, Element, GraphicsPipelineDesc, InputAssemblerDesc, PipelineCreationFlags,
        PrimitiveAssemblerDesc, Rasterizer, Rect, ShaderStageFlags, VertexBufferDesc,
        VertexInputRate, Viewport,
    },
};
use shaderc::Compiler;

pub struct VertexBufferSpec {
    pub attributes: Vec<Format>,
    pub rate: VertexInputRate,
}

impl VertexBufferSpec {
    pub fn as_attribute_desc(&self, binding: BufferIndex) -> Vec<AttributeDesc> {
        let mut v = Vec::with_capacity(self.attributes.len());
        let mut offset = 0;
        for (idx, format) in self.attributes.iter().enumerate() {
            v.push(AttributeDesc {
                location: idx as u32,
                binding,
                element: Element {
                    offset,
                    format: *format,
                },
            });
            offset += get_pixel_size(*format);
        }

        v
    }
    pub fn stride(&self) -> ElemStride {
        self.attributes
            .iter()
            .fold(0, |x, f| x + get_pixel_size(*f))
    }
}

#[derive(Debug, Clone)]
pub struct VertexPrimitiveAssemblerSpec {
    buffers: Vec<VertexBufferDesc>,
    attributes: Vec<AttributeDesc>,
    input_assembler: InputAssemblerDesc,
}

impl VertexPrimitiveAssemblerSpec {
    pub fn with_buffer(&mut self, bd: VertexBufferSpec) -> &mut Self {
        let idx = self.buffers.len() as u32;
        self.buffers.push(VertexBufferDesc {
            binding: idx,
            stride: bd.stride(),
            rate: bd.rate,
        });

        self.attributes.extend(bd.as_attribute_desc(idx));

        self
    }

    pub fn with_buffers(iad: InputAssemblerDesc, mut bds: Vec<VertexBufferSpec>) -> Self {
        let mut this = VertexPrimitiveAssemblerSpec {
            buffers: vec![],
            attributes: vec![],
            input_assembler: iad,
        };

        for bd in bds.drain(..) {
            this.with_buffer(bd);
        }

        this
    }
}

#[derive(Builder, Debug)]
#[builder(public)]
pub struct PipelineSpec {
    rasterizer: Rasterizer,
    depth_stencil: DepthStencilDesc,
    blender: BlendDesc,
    primitive_assembler: VertexPrimitiveAssemblerSpec,

    shader_vertex: ShaderDesc,
    #[builder(setter(strip_option))]
    shader_fragment: Option<ShaderDesc>,
    #[builder(setter(strip_option), default)]
    shader_geom: Option<ShaderDesc>,
    #[builder(setter(strip_option), default)]
    shader_tesselation: Option<(ShaderDesc, ShaderDesc)>,

    #[builder(default = "vec![]")]
    push_constants: Vec<(ShaderStageFlags, Range<u32>)>,

    #[builder(default = "false")]
    dynamic_viewport: bool,
    #[builder(default = "false")]
    dynamic_scissor: bool,

    renderpass: RenderpassSpec,
}

impl PipelineSpec {
    pub fn build<'b, T: Iterator<Item = &'b DescriptorSetLayoutT> + std::fmt::Debug>(
        self,
        device: &mut DeviceT,
        extent: hal::image::Extent,
        set_layouts: T,
    ) -> Result<CompletePipeline> {
        // Renderpass
        let renderpass = self.renderpass.build_renderpass(device)?;

        // Subpass
        let subpass = hal::pass::Subpass {
            index: 0,
            main_pass: &renderpass,
        };

        let mut compiler = Compiler::new().ok_or(EnvironmentError::NoShaderC)?;
        let (vs_module, fs_module, gm_module, ts_module) = {
            (
                self.shader_vertex.compile(&mut compiler, device)?,
                self.shader_fragment
                    .as_ref()
                    .map(|x| x.compile(&mut compiler, device))
                    .transpose()?,
                self.shader_geom
                    .as_ref()
                    .map(|x| x.compile(&mut compiler, device))
                    .transpose()?,
                self.shader_tesselation
                    .as_ref()
                    .map::<Result<_>, _>(|(a, b)| {
                        Ok((
                            a.compile(&mut compiler, device)?,
                            b.compile(&mut compiler, device)?,
                        ))
                    })
                    .transpose()?,
            )
        };

        // Safety: *_module is always populated when shader_* is, so this is safe
        let (vs_entry, fs_entry, gm_entry, ts_entry) = (
            self.shader_vertex.as_entry(&vs_module),
            self.shader_fragment
                .as_ref()
                .map(|x| x.as_entry(fs_module.as_ref().unwrap())),
            self.shader_geom
                .as_ref()
                .map(|x| x.as_entry(gm_module.as_ref().unwrap())),
            self.shader_tesselation.as_ref().map(|(a, b)| {
                (
                    a.as_entry(&ts_module.as_ref().unwrap().0),
                    b.as_entry(&ts_module.as_ref().unwrap().1),
                )
            }),
        );

        // Pipeline layout
        let layout = unsafe {
            device.create_pipeline_layout(set_layouts.into_iter(), self.push_constants.into_iter())
        }
        .context("Error creating pipeline layout")?;

        // Baked states
        let baked_states = BakedStates {
            viewport: match self.dynamic_viewport {
                true => None,
                false => Some(Viewport {
                    rect: extent.rect(),
                    depth: (0.0..1.0),
                }),
            },
            scissor: match self.dynamic_scissor {
                true => None,
                false => Some(extent.rect()),
            },
            blend_constants: None,
            depth_bounds: None,
        };

        // Primitive assembler
        let primitive_assembler = PrimitiveAssemblerDesc::Vertex {
            buffers: self.primitive_assembler.buffers.as_slice(),
            attributes: self.primitive_assembler.attributes.as_slice(),
            input_assembler: self.primitive_assembler.input_assembler,
            vertex: vs_entry,
            tessellation: ts_entry,
            geometry: gm_entry,
        };

        // Pipeline description
        let pipeline_desc = GraphicsPipelineDesc {
            label: Some("stockton"),
            rasterizer: self.rasterizer,
            fragment: fs_entry,
            blender: self.blender,
            depth_stencil: self.depth_stencil,
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
            fs_module,
            gm_module,
            ts_module,
            render_area: extent.rect(),
        })
    }
}

pub struct CompletePipeline {
    /// Our main render pass
    pub renderpass: ManuallyDrop<RenderPassT>,

    /// The layout of our main graphics pipeline
    pub pipeline_layout: ManuallyDrop<PipelineLayoutT>,

    /// Our main graphics pipeline
    pub pipeline: ManuallyDrop<GraphicsPipelineT>,

    /// The vertex shader module
    pub vs_module: ManuallyDrop<ShaderModuleT>,

    /// The fragment shader module
    pub fs_module: Option<ShaderModuleT>,
    pub gm_module: Option<ShaderModuleT>,
    pub ts_module: Option<(ShaderModuleT, ShaderModuleT)>,

    pub render_area: Rect,
}

impl CompletePipeline {
    /// Deactivate vulkan resources. Use before dropping
    pub fn deactivate(mut self, device: &mut DeviceT) {
        unsafe {
            use core::ptr::read;

            device.destroy_render_pass(ManuallyDrop::into_inner(read(&self.renderpass)));

            device.destroy_shader_module(ManuallyDrop::into_inner(read(&self.vs_module)));
            if let Some(x) = self.fs_module.take() {
                device.destroy_shader_module(x)
            }
            if let Some(x) = self.gm_module.take() {
                device.destroy_shader_module(x)
            }
            if let Some((a, b)) = self.ts_module.take() {
                device.destroy_shader_module(a);
                device.destroy_shader_module(b);
            }

            device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));

            device.destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));
        }
    }
}
