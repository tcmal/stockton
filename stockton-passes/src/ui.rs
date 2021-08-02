//! Minimal code for drawing any level, based on traits from stockton-levels

use egui::{ClippedMesh, TextureId};
use hal::{
    buffer::SubRange,
    command::{ClearColor, ClearValue, RenderAttachmentInfo, SubpassContents},
    format::Format,
    image::Layout,
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp},
    pso::{
        BlendDesc, BlendOp, BlendState, ColorBlendDesc, ColorMask, DepthStencilDesc, Face, Factor,
        FrontFace, InputAssemblerDesc, LogicOp, PolygonMode, Primitive, Rasterizer, Rect,
        ShaderStageFlags, State, VertexInputRate,
    },
};
use shaderc::ShaderKind;
use stockton_render::{
    buffers::{DrawBuffers, ModifiableBuffer},
    builders::{
        CompletePipeline, PipelineSpecBuilder, RenderpassSpec, ShaderDesc, VertexBufferSpec,
        VertexPrimitiveAssemblerSpec,
    },
    context::RenderingContext,
    draw_passes::{util::TargetSpecificResources, DrawPass, IntoDrawPass},
    error::{EnvironmentError, LockPoisoned},
    queue_negotiator::QueueNegotiator,
    texture::{TexLoadQueue, TextureLoadConfig, TextureRepo},
    types::*,
};
use stockton_types::{Session, Vector2};

use std::{
    array::IntoIter,
    convert::TryInto,
    iter::{empty, once},
};

use anyhow::{anyhow, Context, Result};
use egui::{CtxRef, Texture};
use std::sync::Arc;
use stockton_render::texture::{resolver::TextureResolver, LoadableImage};

use crate::window::UiState;

#[derive(Debug)]
pub struct UiPoint(pub Vector2, pub Vector2, pub [f32; 4]);

/// Draw a Ui object
pub struct UiDrawPass<'a> {
    pipeline: CompletePipeline,
    repo: TextureRepo,
    draw_buffers: DrawBuffers<'a, UiPoint>,

    framebuffers: TargetSpecificResources<FramebufferT>,
}

impl<'a> DrawPass for UiDrawPass<'a> {
    fn queue_draw(
        &mut self,
        session: &Session,
        img_view: &ImageViewT,
        cmd_buffer: &mut CommandBufferT,
    ) -> anyhow::Result<()> {
        // We might have loaded more textures
        self.repo.process_responses();

        // Make sure we update the vertex buffers after they're written to, but before they're read from.
        self.draw_buffers
            .vertex_buffer
            .record_commit_cmds(cmd_buffer)?;
        self.draw_buffers
            .index_buffer
            .record_commit_cmds(cmd_buffer)?;

        // Get level & camera
        let ui: &mut UiState = &mut session.resources.get_mut::<UiState>().unwrap();

        // Get framebuffer and depth buffer
        let fb = self.framebuffers.get_next();
        unsafe {
            cmd_buffer.begin_render_pass(
                &self.pipeline.renderpass,
                fb,
                self.pipeline.render_area,
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
            cmd_buffer.bind_graphics_pipeline(&self.pipeline.pipeline);

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

        let (_out, shapes) = ui.end_frame();
        let screen = ui
            .dimensions()
            .ok_or_else(|| anyhow!("UI not set up properly."))?;
        let shapes = ui.ctx().tessellate(shapes);

        for ClippedMesh(rect, tris) in shapes.iter() {
            assert!(tris.texture_id == TextureId::Egui);

            // Copy triangles/indicies
            for i in (0..tris.indices.len()).step_by(3) {
                self.draw_buffers.index_buffer[i / 3] = (
                    tris.indices[i].try_into()?,
                    tris.indices[i + 1].try_into()?,
                    tris.indices[i + 2].try_into()?,
                );
            }
            for (i, vertex) in tris.vertices.iter().enumerate() {
                self.draw_buffers.vertex_buffer[i] = UiPoint(
                    Vector2::new(vertex.pos.x, vertex.pos.y),
                    Vector2::new(vertex.uv.x, vertex.uv.y),
                    [
                        vertex.color.r() as f32 / 255.0,
                        vertex.color.g() as f32 / 255.0,
                        vertex.color.b() as f32 / 255.0,
                        vertex.color.a() as f32 / 255.0,
                    ],
                );
            }
            // TODO: *Properly* deal with textures
            if let Some(ds) = self.repo.attempt_get_descriptor_set(0) {
                unsafe {
                    cmd_buffer.push_graphics_constants(
                        &self.pipeline.pipeline_layout,
                        ShaderStageFlags::VERTEX,
                        0,
                        &[screen.x.to_bits(), screen.y.to_bits()],
                    );

                    cmd_buffer.set_scissors(
                        0,
                        IntoIter::new([Rect {
                            x: rect.min.x as i16,
                            y: rect.min.y as i16,
                            w: rect.width() as i16,
                            h: rect.height() as i16,
                        }]),
                    );
                    cmd_buffer.bind_graphics_descriptor_sets(
                        &self.pipeline.pipeline_layout,
                        0,
                        IntoIter::new([ds]),
                        empty(),
                    );
                    // Call draw
                    cmd_buffer.draw_indexed(0..tris.indices.len() as u32, 0, 0..1);
                }
            } else {
                self.repo.queue_load(0)?;
            }
        }

        unsafe {
            cmd_buffer.end_render_pass();
        }

        Ok(())
    }

    fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        unsafe {
            let mut device = context.device().write().map_err(|_| LockPoisoned::Device)?;
            self.pipeline.deactivate(&mut device);
            self.draw_buffers.deactivate(&mut device);
            for fb in self.framebuffers.dissolve() {
                device.destroy_framebuffer(fb);
            }
        }
        self.repo.deactivate(context.device());

        Ok(())
    }

    fn handle_surface_change(
        &mut self,
        _session: &Session,
        _context: &mut RenderingContext,
    ) -> Result<()> {
        todo!()
    }
}

impl<'a> IntoDrawPass<UiDrawPass<'a>> for () {
    fn init(self, session: &mut Session, context: &mut RenderingContext) -> Result<UiDrawPass<'a>> {
        let spec = PipelineSpecBuilder::default()
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
                    attributes: vec![Format::Rg32Sfloat, Format::Rg32Sfloat, Format::Rgba32Sfloat],
                    rate: VertexInputRate::Vertex,
                }],
            ))
            .shader_vertex(ShaderDesc {
                source: include_str!("./data/ui.vert").to_string(),
                entry: "main".to_string(),
                kind: ShaderKind::Vertex,
            })
            .shader_fragment(ShaderDesc {
                source: include_str!("./data/ui.frag").to_string(),
                entry: "main".to_string(),
                kind: ShaderKind::Fragment,
            })
            .push_constants(vec![(ShaderStageFlags::VERTEX, 0..8)])
            .renderpass(RenderpassSpec {
                colors: vec![Attachment {
                    format: Some(context.target_chain().properties().format),
                    samples: 1,
                    ops: AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
                    stencil_ops: AttachmentOps::new(
                        AttachmentLoadOp::DontCare,
                        AttachmentStoreOp::DontCare,
                    ),
                    layouts: Layout::ColorAttachmentOptimal..Layout::ColorAttachmentOptimal,
                }],
                depth: None,
                inputs: vec![],
                resolves: vec![],
                preserves: vec![],
            })
            .dynamic_scissor(true)
            .build()
            .context("Error building pipeline")?;

        let ui: &mut UiState = &mut session.resources.get_mut::<UiState>().unwrap();
        let repo = TextureRepo::new(
            context.device().clone(),
            context
                .queue_negotiator_mut()
                .family::<TexLoadQueue>()
                .ok_or(EnvironmentError::NoSuitableFamilies)
                .context("Error finding texture queue")?,
            context
                .queue_negotiator_mut()
                .get_queue::<TexLoadQueue>()
                .ok_or(EnvironmentError::NoQueues)
                .context("Error finding texture queue")?,
            context.adapter(),
            TextureLoadConfig {
                resolver: UiTextures::new(ui.ctx().clone()),
                filter: hal::image::Filter::Linear,
                wrap_mode: hal::image::WrapMode::Clamp,
            },
        )
        .context("Error creating texture repo")?;

        let (draw_buffers, pipeline, framebuffers) = {
            let mut device = context.device().write().map_err(|_| LockPoisoned::Device)?;
            let draw_buffers = DrawBuffers::new(&mut device, context.adapter())
                .context("Error creating draw buffers")?;
            let pipeline = spec
                .build(
                    &mut device,
                    context.target_chain().properties().extent,
                    context.target_chain().properties(),
                    once(&*repo.get_ds_layout()?),
                )
                .context("Error building pipeline")?;

            let fat = context.target_chain().properties().framebuffer_attachment();
            let framebuffers = TargetSpecificResources::new(
                || unsafe {
                    Ok(device.create_framebuffer(
                        &pipeline.renderpass,
                        IntoIter::new([fat.clone()]),
                        context.target_chain().properties().extent,
                    )?)
                },
                context.target_chain().properties().image_count as usize,
            )?;
            (draw_buffers, pipeline, framebuffers)
        };

        Ok(UiDrawPass {
            pipeline,
            repo,
            draw_buffers,
            framebuffers,
        })
    }

    fn find_aux_queues<'c>(
        adapter: &'c Adapter,
        queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'c QueueFamilyT, Vec<f32>)>> {
        queue_negotiator.find(adapter, &TexLoadQueue)?;

        Ok(vec![queue_negotiator
            .family_spec::<TexLoadQueue>(&adapter.queue_families, 1)
            .ok_or(EnvironmentError::NoSuitableFamilies)?])
    }
}

pub struct UiTexture(Arc<Texture>);

pub struct UiTextures {
    ctx: CtxRef,
}

impl TextureResolver for UiTextures {
    type Image = UiTexture;
    fn resolve(&mut self, tex: u32) -> Option<Self::Image> {
        if tex == 0 {
            Some(UiTexture(self.ctx.texture()))
        } else {
            None
        }
    }
}

impl UiTextures {
    pub fn new(ctx: CtxRef) -> Self {
        UiTextures { ctx }
    }
}

impl LoadableImage for UiTexture {
    fn width(&self) -> u32 {
        self.0.width as u32
    }
    fn height(&self) -> u32 {
        self.0.height as u32
    }
    unsafe fn copy_row(&self, y: u32, ptr: *mut u8) {
        let row_size = self.0.width as u32;
        let pixels = &self.0.pixels[(y * row_size) as usize..((y + 1) * row_size) as usize];

        for (i, x) in pixels.iter().enumerate() {
            *ptr.offset(i as isize * 4) = 255;
            *ptr.offset((i as isize * 4) + 1) = 255;
            *ptr.offset((i as isize * 4) + 2) = 255;
            *ptr.offset((i as isize * 4) + 3) = *x;
        }
    }
}