//! Minimal code for drawing any level, based on traits from stockton-levels

use anyhow::{Context, Result};
use hal::{
    buffer::SubRange,
    command::{ClearColor, ClearValue, RenderAttachmentInfo, SubpassContents},
    format::Format,
    image::Layout,
    pass::Attachment,
    pso::{
        BlendDesc, BlendOp, BlendState, ColorBlendDesc, ColorMask, DepthStencilDesc, Face, Factor,
        FrontFace, InputAssemblerDesc, LogicOp, PolygonMode, Primitive, Rasterizer, State,
        VertexInputRate,
    },
};
use legion::{Entity, IntoQuery};
use std::{
    array::IntoIter,
    iter::{empty, once},
};
use stockton_skeleton::{
    buffers::draw::DrawBuffers,
    builders::{
        AttachmentSpec, CompletePipeline, PipelineSpecBuilder, RenderpassSpec, ShaderDesc,
        ShaderKind, VertexBufferSpec, VertexPrimitiveAssemblerSpec,
    },
    draw_passes::util::TargetSpecificResources,
    mem::{DataPool, StagingPool},
    queue_negotiator::QueueFamilyNegotiator,
    types::*,
    DrawPass, IntoDrawPass, PassPosition, RenderingContext, Session,
};

use crate::ExampleState;

/// The vertices that go to the shader (XY + RGB)
#[derive(Debug, Clone, Copy)]
#[repr(C)]
struct Vertex(pub Vector2, pub Vector3);

/// An example draw pass
pub struct ExampleDrawPass<'a> {
    /// Index and vertex buffer pair
    draw_buffers: DrawBuffers<'a, Vertex, DataPool, StagingPool>,

    /// Resources that depend on the surface. This is seperate so that we can deal with surface changes more easily.
    surface_resources: SurfaceDependentResources,

    /// Entity we get our state from
    state_ent: Entity,
}

/// Config for our draw pass. This is turned into our drawpass using [`IntoDrawPass`]
pub struct ExampleDrawPassConfig {
    pub state_ent: Entity,
}

impl<'a, P: PassPosition> DrawPass<P> for ExampleDrawPass<'a> {
    /// Called every frame to queue actual drawing.
    fn queue_draw(
        &mut self,
        session: &Session,
        img_view: &ImageViewT,
        cmd_buffer: &mut CommandBufferT,
    ) -> anyhow::Result<()> {
        // Commit any changes to our vertex buffers
        // We queue this first so that it's executed before any draw commands
        self.draw_buffers
            .vertex_buffer
            .record_commit_cmds(cmd_buffer)?;
        self.draw_buffers
            .index_buffer
            .record_commit_cmds(cmd_buffer)?;

        // Get framebuffer
        let fb = self.surface_resources.framebuffers.get_next();

        // Get state
        let (state,) = <(&ExampleState,)>::query().get(&session.world, self.state_ent)?;

        // Begin render pass & bind everything needed
        unsafe {
            cmd_buffer.begin_render_pass(
                &self.surface_resources.pipeline.renderpass,
                fb,
                self.surface_resources.pipeline.render_area,
                vec![RenderAttachmentInfo {
                    image_view: img_view,
                    clear_value: ClearValue {
                        color: ClearColor {
                            float32: [0.0, 0.0, 0.0, 1.0],
                        },
                    },
                }]
                .into_iter(),
                SubpassContents::Inline,
            );
            cmd_buffer.bind_graphics_pipeline(&self.surface_resources.pipeline.pipeline);

            // Bind buffers
            cmd_buffer.bind_vertex_buffers(
                0,
                once((
                    self.draw_buffers.vertex_buffer.get_buffer(),
                    SubRange {
                        offset: 0,
                        size: None,
                    },
                )),
            );
            cmd_buffer.bind_index_buffer(
                self.draw_buffers.index_buffer.get_buffer(),
                SubRange {
                    offset: 0,
                    size: None,
                },
                hal::IndexType::U16,
            );
        }

        // Draw an example
        self.draw_buffers.index_buffer[0] = (0, 1, 2);
        self.draw_buffers.vertex_buffer[0] = Vertex(Vector2::new(0.5, 0.5), state.color());
        self.draw_buffers.vertex_buffer[1] = Vertex(Vector2::new(0.0, -0.5), state.color());
        self.draw_buffers.vertex_buffer[2] = Vertex(Vector2::new(-0.5, 0.5), state.color());

        unsafe {
            cmd_buffer.draw_indexed(0..3, 0, 0..1);
        }

        // Remember to clean up afterwards!
        unsafe {
            cmd_buffer.end_render_pass();
        }

        Ok(())
    }

    /// Destroy all our vulkan objects
    fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        self.draw_buffers.deactivate(context);
        self.surface_resources.deactivate(context)?;

        Ok(())
    }

    /// Deal with a surface change
    fn handle_surface_change(
        mut self,
        _session: &Session,
        context: &mut RenderingContext,
    ) -> Result<Self> {
        // We need to make sure there's never an invalid value for self.surface_resources,
        // and that we deactivate everything in case of an error (since we'll be dropped in that case).
        let new_resources = match SurfaceDependentResources::new::<P>(context) {
            Ok(x) => x,
            Err(e) => {
                <Self as DrawPass<P>>::deactivate(self, context)?;

                return Err(e);
            }
        };

        let old_resources = self.surface_resources;
        self.surface_resources = new_resources;

        match old_resources.deactivate(context) {
            Ok(_) => Ok(self),
            Err(e) => {
                <Self as DrawPass<P>>::deactivate(self, context)?;
                Err(e)
            }
        }
    }
}

impl<'a, P: PassPosition> IntoDrawPass<ExampleDrawPass<'a>, P> for ExampleDrawPassConfig {
    /// Create our example draw pass
    fn init(
        self,
        _session: &mut Session,
        context: &mut RenderingContext,
    ) -> Result<ExampleDrawPass<'a>> {
        let surface_resources = SurfaceDependentResources::new::<P>(context)?;
        let draw_buffers =
            match DrawBuffers::from_context(context).context("Error creating draw buffers") {
                Ok(x) => x,
                Err(e) => {
                    surface_resources.deactivate(context)?;
                    return Err(e);
                }
            };

        Ok(ExampleDrawPass {
            draw_buffers,
            surface_resources,
            state_ent: self.state_ent,
        })
    }

    fn find_aux_queues(
        _adapter: &Adapter,
        _queue_negotiator: &mut QueueFamilyNegotiator,
    ) -> Result<()> {
        // We don't need any queues, but we'd need code to find their families here if we did.
        Ok(())
    }
}

/// Used to store resources which depend on the surface, for convenience in handle_surface_change
struct SurfaceDependentResources {
    pub pipeline: CompletePipeline,
    pub framebuffers: TargetSpecificResources<FramebufferT>,
}

impl SurfaceDependentResources {
    pub fn new<P: PassPosition>(context: &mut RenderingContext) -> Result<Self> {
        let (pipeline, framebuffers) = {
            // Our graphics pipeline
            // Vulkan has a lot of config, so this is basically always going to be a big builder block
            let pipeline_spec = PipelineSpecBuilder::default()
                .rasterizer(Rasterizer {
                    polygon_mode: PolygonMode::Fill,
                    cull_face: Face::NONE,
                    front_face: FrontFace::CounterClockwise,
                    depth_clamping: false,
                    depth_bias: None,
                    conservative: true,
                    line_width: State::Static(1.0),
                })
                .depth_stencil(DepthStencilDesc {
                    depth: None,
                    depth_bounds: false,
                    stencil: None,
                })
                .blender(BlendDesc {
                    logic_op: Some(LogicOp::Copy),
                    targets: vec![ColorBlendDesc {
                        mask: ColorMask::ALL,
                        blend: Some(BlendState {
                            color: BlendOp::Add {
                                src: Factor::SrcAlpha,
                                dst: Factor::OneMinusSrcAlpha,
                            },
                            alpha: BlendOp::Add {
                                src: Factor::SrcAlpha,
                                dst: Factor::OneMinusSrcAlpha,
                            },
                        }),
                    }],
                })
                .primitive_assembler(VertexPrimitiveAssemblerSpec::with_buffers(
                    InputAssemblerDesc::new(Primitive::TriangleList),
                    vec![VertexBufferSpec {
                        attributes: vec![Format::Rg32Sfloat, Format::Rgb32Sfloat],
                        rate: VertexInputRate::Vertex,
                    }],
                ))
                .shader_vertex(ShaderDesc {
                    source: include_str!("./data/shader.vert").to_string(),
                    entry: "main".to_string(),
                    kind: ShaderKind::Vertex,
                })
                .shader_fragment(ShaderDesc {
                    source: include_str!("./data/shader.frag").to_string(),
                    entry: "main".to_string(),
                    kind: ShaderKind::Fragment,
                })
                .renderpass(RenderpassSpec {
                    colors: vec![AttachmentSpec {
                        attachment: Attachment {
                            format: Some(context.properties().color_format),
                            samples: 1,
                            // Here we use PassPosition to get the proper operations
                            // Since, for example, the last pass needs to finish in present mode.
                            ops: P::attachment_ops(),
                            stencil_ops: P::attachment_ops(),
                            layouts: P::layout_as_range(),
                        },
                        // This is the layout we want to deal with in our `queue_draw` function.
                        // It's almost certainly `Layout::ColorAttachmentOptimal`
                        used_layout: Layout::ColorAttachmentOptimal,
                    }],
                    depth: None,
                    inputs: vec![],
                    resolves: vec![],
                    preserves: vec![],
                })
                .build()
                .context("Error building pipeline")?;

            // Lock our device to actually build it
            // Try to lock the device for as little time as possible
            let mut device = context.lock_device()?;

            let pipeline = pipeline_spec
                .build(&mut device, context.properties().extent, empty())
                .context("Error building pipeline")?;

            // Our framebuffers just have the swapchain framebuffer attachment
            // TargetSpecificResources makes sure we use a different one each frame.
            let fat = context.properties().swapchain_framebuffer_attachment();
            let framebuffers = TargetSpecificResources::new(
                || unsafe {
                    Ok(device.create_framebuffer(
                        &pipeline.renderpass,
                        IntoIter::new([fat.clone()]),
                        context.properties().extent,
                    )?)
                },
                context.properties().image_count as usize,
            )?;

            (pipeline, framebuffers)
        };

        Ok(Self {
            pipeline,
            framebuffers,
        })
    }

    pub fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        unsafe {
            let mut device = context.lock_device()?;
            for fb in self.framebuffers.dissolve() {
                device.destroy_framebuffer(fb);
            }

            self.pipeline.deactivate(&mut device);
        }

        Ok(())
    }
}
