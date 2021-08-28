//! Minimal code for drawing any level, based on traits from stockton-levels
use crate::window::UiState;

use stockton_skeleton::{
    buffers::draw::DrawBuffers,
    builders::{
        AttachmentSpec, CompletePipeline, PipelineSpecBuilder, RenderpassSpec, ShaderDesc,
        VertexBufferSpec, VertexPrimitiveAssemblerSpec,
    },
    context::RenderingContext,
    draw_passes::{util::TargetSpecificResources, DrawPass, IntoDrawPass, PassPosition},
    mem::{DataPool, StagingPool, TexturesPool},
    queue_negotiator::QueueFamilyNegotiator,
    texture::{
        resolver::TextureResolver, LoadableImage, TexLoadQueue, TextureLoadConfig, TextureRepo,
    },
    types::*,
};
use stockton_types::{Session, Vector2};

use std::{
    array::IntoIter,
    convert::TryInto,
    iter::{empty, once},
    sync::Arc,
};

use anyhow::{anyhow, Context, Result};
use egui::{ClippedMesh, TextureId};
use egui::{CtxRef, Texture};
use hal::{
    buffer::SubRange,
    command::{ClearColor, ClearValue, RenderAttachmentInfo, SubpassContents},
    format::Format,
    image::Layout,
    pass::Attachment,
    pso::{
        BlendDesc, BlendOp, BlendState, ColorBlendDesc, ColorMask, DepthStencilDesc, Face, Factor,
        FrontFace, InputAssemblerDesc, LogicOp, PolygonMode, Primitive, Rasterizer, Rect,
        ShaderStageFlags, State, VertexInputRate,
    },
};
use shaderc::ShaderKind;

#[derive(Debug)]
pub struct UiPoint(pub Vector2, pub Vector2, pub [f32; 4]);

/// Draw a Ui object
pub struct UiDrawPass<'a> {
    repo: TextureRepo<TexturesPool, StagingPool>,
    draw_buffers: DrawBuffers<'a, UiPoint, DataPool, StagingPool>,

    surface_resources: SurfaceDependentResources,
}

impl<'a, P: PassPosition> DrawPass<P> for UiDrawPass<'a> {
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
        let fb = self.surface_resources.framebuffers.get_next();
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

        let (_out, shapes) = ui.end_frame();
        let screen = ui
            .dimensions()
            .ok_or_else(|| anyhow!("UI not set up properly."))?;
        let shapes = ui.ctx().tessellate(shapes);

        let mut next_idx_idx = 0;
        let mut next_vert_idx = 0;
        for ClippedMesh(rect, tris) in shapes.iter() {
            assert!(tris.texture_id == TextureId::Egui);

            // Copy triangles/indicies
            for i in (0..tris.indices.len()).step_by(3) {
                self.draw_buffers.index_buffer[next_idx_idx + (i / 3)] = (
                    tris.indices[i].try_into()?,
                    tris.indices[i + 1].try_into()?,
                    tris.indices[i + 2].try_into()?,
                );
            }

            for (i, vertex) in tris.vertices.iter().enumerate() {
                self.draw_buffers.vertex_buffer[next_vert_idx + i] = UiPoint(
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
                        &self.surface_resources.pipeline.pipeline_layout,
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
                        &self.surface_resources.pipeline.pipeline_layout,
                        0,
                        IntoIter::new([ds]),
                        empty(),
                    );
                    // Call draw
                    cmd_buffer.draw_indexed(
                        (next_idx_idx as u32 * 3)..((next_idx_idx * 3) + tris.indices.len()) as u32,
                        next_vert_idx as i32,
                        0..1,
                    );
                }
            } else {
                self.repo.queue_load(0)?;
            }

            next_idx_idx += tris.indices.len() / 3;
            next_vert_idx += tris.vertices.len();
        }

        unsafe {
            cmd_buffer.end_render_pass();
        }

        Ok(())
    }

    fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        self.draw_buffers.deactivate(context);
        self.surface_resources.deactivate(context)?;
        self.repo.deactivate(context);

        Ok(())
    }

    fn handle_surface_change(
        mut self,
        _session: &Session,
        context: &mut RenderingContext,
    ) -> Result<Self> {
        let new_surface_resources =
            SurfaceDependentResources::new::<P>(context, &*self.repo.get_ds_layout()?)?;
        let old_surface_resources = self.surface_resources;
        self.surface_resources = new_surface_resources;

        match old_surface_resources.deactivate(context) {
            Ok(_) => Ok(self),
            Err(e) => {
                <Self as DrawPass<P>>::deactivate(self, context)?;
                Err(e)
            }
        }
    }
}

impl<'a, P: PassPosition> IntoDrawPass<UiDrawPass<'a>, P> for () {
    fn init(self, session: &mut Session, context: &mut RenderingContext) -> Result<UiDrawPass<'a>> {
        let ui: &mut UiState = &mut session.resources.get_mut::<UiState>().unwrap();
        let repo = TextureRepo::new::<_, TexLoadQueue>(
            context,
            TextureLoadConfig {
                resolver: UiTextures::new(ui.ctx().clone()),
                filter: hal::image::Filter::Linear,
                wrap_mode: hal::image::WrapMode::Clamp,
            },
        )
        .context("Error creating texture repo")?;

        let draw_buffers =
            DrawBuffers::from_context(context).context("Error creating draw buffers")?;

        let surface_resources =
            SurfaceDependentResources::new::<P>(context, &*repo.get_ds_layout()?)?;

        Ok(UiDrawPass {
            repo,
            draw_buffers,
            surface_resources,
        })
    }

    fn find_aux_queues(
        adapter: &Adapter,
        queue_negotiator: &mut QueueFamilyNegotiator,
    ) -> Result<()> {
        queue_negotiator.find(adapter, &TexLoadQueue, 1)?;

        Ok(())
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

struct SurfaceDependentResources {
    pipeline: CompletePipeline,
    framebuffers: TargetSpecificResources<FramebufferT>,
}

impl SurfaceDependentResources {
    fn new<P: PassPosition>(
        context: &mut RenderingContext,
        ds_layout: &DescriptorSetLayoutT,
    ) -> Result<Self> {
        let mut device = context.lock_device()?;

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
                colors: vec![AttachmentSpec {
                    attachment: Attachment {
                        format: Some(context.properties().color_format),
                        samples: 1,
                        ops: P::attachment_ops(),
                        stencil_ops: P::attachment_ops(),
                        layouts: P::layout_as_range(),
                    },
                    used_layout: Layout::ColorAttachmentOptimal,
                }],
                depth: None,
                inputs: vec![],
                resolves: vec![],
                preserves: vec![],
            })
            .dynamic_scissor(true)
            .build()
            .context("Error building pipeline")?;

        let pipeline = spec
            .build(&mut device, context.properties().extent, once(ds_layout))
            .context("Error building pipeline")?;

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

        Ok(Self {
            framebuffers,
            pipeline,
        })
    }

    fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        unsafe {
            let mut device = context.lock_device()?;
            self.pipeline.deactivate(&mut device);

            for fb in self.framebuffers.dissolve() {
                device.destroy_framebuffer(fb);
            }
        }

        Ok(())
    }
}
