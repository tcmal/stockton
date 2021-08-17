//! Minimal code for drawing any level, based on traits from stockton-levels

use stockton_levels::{
    features::MinRenderFeatures,
    parts::{data::Geometry, IsFace},
};
use stockton_skeleton::{
    buffers::{
        draw::{DrawBuffers, INITIAL_INDEX_SIZE, INITIAL_VERT_SIZE},
        image::{BoundImageView, ImageSpec, DEPTH_RESOURCES},
    },
    builders::{
        AttachmentSpec, CompletePipeline, PipelineSpecBuilder, RenderpassSpec, ShaderDesc,
        VertexBufferSpec, VertexPrimitiveAssemblerSpec,
    },
    context::RenderingContext,
    draw_passes::{util::TargetSpecificResources, DrawPass, IntoDrawPass, PassPosition},
    error::LockPoisoned,
    mem::{DataPool, DepthBufferPool, StagingPool, TexturesPool},
    queue_negotiator::QueueNegotiator,
    texture::{resolver::TextureResolver, TexLoadQueue, TextureLoadConfig, TextureRepo},
    types::*,
};
use stockton_types::{
    components::{CameraSettings, CameraVPMatrix, Transform},
    *,
};

use std::{
    array::IntoIter,
    convert::TryInto,
    iter::{empty, once},
    marker::PhantomData,
    sync::{Arc, RwLock},
};

use anyhow::{Context, Result};
use hal::{
    buffer::SubRange,
    command::{ClearColor, ClearDepthStencil, ClearValue, RenderAttachmentInfo, SubpassContents},
    format::Format,
    image::{Filter, FramebufferAttachment, Layout, Usage, ViewCapabilities, WrapMode},
    pass::{Attachment, AttachmentLoadOp, AttachmentOps, AttachmentStoreOp},
    pso::{
        BlendDesc, BlendOp, BlendState, ColorBlendDesc, ColorMask, Comparison, DepthStencilDesc,
        DepthTest, Face, Factor, FrontFace, InputAssemblerDesc, LogicOp, PolygonMode, Primitive,
        Rasterizer, ShaderStageFlags, State, VertexInputRate,
    },
};
use legion::{Entity, IntoQuery};
use shaderc::ShaderKind;
use thiserror::Error;

/// The Vertexes that go to the shader
#[derive(Debug, Clone, Copy)]
struct UvPoint(pub Vector3, pub i32, pub Vector2);

/// Draw a level
pub struct LevelDrawPass<'a, M> {
    pipeline: CompletePipeline,
    repo: TextureRepo<TexturesPool, StagingPool>,
    active_camera: Entity,
    draw_buffers: DrawBuffers<'a, UvPoint, DataPool, StagingPool>,

    framebuffers: TargetSpecificResources<FramebufferT>,
    depth_buffers: TargetSpecificResources<BoundImageView<DepthBufferPool>>,

    _d: PhantomData<M>,
}

impl<'a, M, P: PassPosition> DrawPass<P> for LevelDrawPass<'a, M>
where
    M: for<'b> MinRenderFeatures<'b> + 'static,
{
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
        let mut query = <(&Transform, &CameraSettings, &CameraVPMatrix)>::query();
        let (camera_transform, camera_settings, camera_vp) = query
            .get(&session.world, self.active_camera)
            .context("Couldn't find camera components")?;
        let map_lock: Arc<RwLock<M>> = session.resources.get::<Arc<RwLock<M>>>().unwrap().clone();
        let map = map_lock.read().map_err(|_| LockPoisoned::Map)?;

        // Get framebuffer and depth buffer
        let fb = self.framebuffers.get_next();
        let db = self.depth_buffers.get_next();

        unsafe {
            cmd_buffer.begin_render_pass(
                &self.pipeline.renderpass,
                fb,
                self.pipeline.render_area,
                vec![
                    RenderAttachmentInfo {
                        image_view: img_view,
                        clear_value: ClearValue {
                            color: ClearColor {
                                float32: [0.0, 0.0, 0.0, 1.0],
                            },
                        },
                    },
                    RenderAttachmentInfo {
                        image_view: &*db.img_view(),
                        clear_value: ClearValue {
                            depth_stencil: ClearDepthStencil {
                                depth: 1.0,
                                stencil: 0,
                            },
                        },
                    },
                ]
                .into_iter(),
                SubpassContents::Inline,
            );
            cmd_buffer.bind_graphics_pipeline(&self.pipeline.pipeline);

            // VP Matrix
            let vp = &*(camera_vp.vp_matrix.data.as_slice() as *const [f32] as *const [u32]);

            cmd_buffer.push_graphics_constants(
                &self.pipeline.pipeline_layout,
                ShaderStageFlags::VERTEX,
                0,
                vp,
            );

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

        // Get visible faces
        let mut faces = map.get_visible(camera_transform, camera_settings);

        // Iterate over faces, copying them in and drawing groups that use the same texture chunk all at once.
        let face = faces.next();
        if let Some(face) = face {
            let mut face = map.get_face(face).ok_or(LevelError::BadReference)?;
            let mut current_chunk = face.texture_idx(&map) as usize / 8;
            let mut chunk_start = 0;

            let mut curr_vert_idx: usize = 0;
            let mut curr_idx_idx: usize = 0;
            loop {
                if current_chunk != face.texture_idx(&map) as usize / 8 {
                    // Last index was last of group, so draw it all if textures are loaded.
                    self.draw_or_queue(
                        current_chunk,
                        cmd_buffer,
                        chunk_start as u32,
                        curr_idx_idx as u32,
                    )?;

                    // Next group of same-chunked faces starts here.
                    chunk_start = curr_idx_idx;
                    current_chunk = face.texture_idx(&map) as usize / 8;
                }

                match face.geometry(&map) {
                    Geometry::Vertices(v1, v2, v3) => {
                        for v in &[v1, v2, v3] {
                            let uvp =
                                UvPoint(v.position, face.texture_idx(&map).try_into()?, v.tex);

                            self.draw_buffers.vertex_buffer[curr_vert_idx] = uvp;
                            curr_vert_idx += 1;
                        }
                        self.draw_buffers.index_buffer[curr_idx_idx] = (
                            curr_vert_idx as u16 - 3,
                            curr_vert_idx as u16 - 2,
                            curr_vert_idx as u16 - 1,
                        );
                        curr_idx_idx += 1;
                    }
                }

                if curr_vert_idx >= INITIAL_VERT_SIZE.try_into()?
                    || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into()?
                {
                    println!("out of vertex buffer space!");
                    break;
                }

                match faces.next() {
                    Some(x) => face = map.get_face(x).ok_or(LevelError::BadReference)?,
                    None => break,
                };
            }

            // Draw the final group of chunks
            self.draw_or_queue(
                current_chunk,
                cmd_buffer,
                chunk_start as u32,
                curr_idx_idx as u32,
            )?;
        }

        unsafe {
            cmd_buffer.end_render_pass();
        }

        Ok(())
    }

    fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        self.draw_buffers.deactivate(context);
        unsafe {
            let mut device = context.device().write().map_err(|_| LockPoisoned::Device)?;
            self.pipeline.deactivate(&mut device);
            for fb in self.framebuffers.dissolve() {
                device.destroy_framebuffer(fb);
            }
        }
        for db in self.depth_buffers.dissolve() {
            db.deactivate_with_context(context);
        }
        self.repo.deactivate(context);

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
impl<'a, M> LevelDrawPass<'a, M> {
    fn draw_or_queue(
        &mut self,
        current_chunk: usize,
        cmd_buffer: &mut CommandBufferT,
        chunk_start: u32,
        curr_idx_idx: u32,
    ) -> Result<()> {
        if let Some(ds) = self.repo.attempt_get_descriptor_set(current_chunk) {
            unsafe {
                cmd_buffer.bind_graphics_descriptor_sets(
                    &*self.pipeline.pipeline_layout,
                    0,
                    once(ds),
                    empty(),
                );
                cmd_buffer.draw_indexed(chunk_start * 3..(curr_idx_idx * 3) + 1, 0, 0..1);
            }
        } else {
            self.repo.queue_load(current_chunk)?
        }

        Ok(())
    }
}

pub struct LevelDrawPassConfig<R> {
    pub active_camera: Entity,
    pub tex_resolver: R,
}

impl<'a, M, R, P> IntoDrawPass<LevelDrawPass<'a, M>, P> for LevelDrawPassConfig<R>
where
    M: for<'b> MinRenderFeatures<'b> + 'static,
    R: TextureResolver + Send + Sync + 'static,
    P: PassPosition,
{
    fn init(
        self,
        _session: &mut Session,
        context: &mut RenderingContext,
    ) -> Result<LevelDrawPass<'a, M>> {
        let spec = PipelineSpecBuilder::default()
            .rasterizer(Rasterizer {
                polygon_mode: PolygonMode::Fill,
                cull_face: Face::BACK,
                front_face: FrontFace::CounterClockwise,
                depth_clamping: false,
                depth_bias: None,
                conservative: true,
                line_width: State::Static(1.0),
            })
            .depth_stencil(DepthStencilDesc {
                depth: Some(DepthTest {
                    fun: Comparison::Less,
                    write: true,
                }),
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
                    attributes: vec![Format::Rgb32Sfloat, Format::R32Sint, Format::Rg32Sfloat],
                    rate: VertexInputRate::Vertex,
                }],
            ))
            .shader_vertex(ShaderDesc {
                source: include_str!("./data/3d.vert").to_string(),
                entry: "main".to_string(),
                kind: ShaderKind::Vertex,
            })
            .shader_fragment(ShaderDesc {
                source: include_str!("./data/3d.frag").to_string(),
                entry: "main".to_string(),
                kind: ShaderKind::Fragment,
            })
            .push_constants(vec![(ShaderStageFlags::VERTEX, 0..64)])
            .renderpass(RenderpassSpec {
                colors: vec![AttachmentSpec {
                    attachment: Attachment {
                        format: Some(context.target_chain().properties().format),
                        samples: 1,
                        ops: P::attachment_ops(),
                        stencil_ops: P::attachment_ops(),
                        layouts: P::layout_as_range(),
                    },

                    used_layout: Layout::ColorAttachmentOptimal,
                }],
                depth: Some(AttachmentSpec {
                    attachment: Attachment {
                        format: Some(context.target_chain().properties().depth_format),
                        samples: 1,
                        ops: AttachmentOps::new(
                            AttachmentLoadOp::Clear,
                            AttachmentStoreOp::DontCare,
                        ),
                        stencil_ops: AttachmentOps::new(
                            AttachmentLoadOp::DontCare,
                            AttachmentStoreOp::DontCare,
                        ),
                        layouts: Layout::Undefined..Layout::DepthStencilAttachmentOptimal,
                    },
                    used_layout: Layout::DepthStencilAttachmentOptimal,
                }),
                inputs: vec![],
                resolves: vec![],
                preserves: vec![],
            })
            .build()
            .context("Error building pipeline")?;

        let repo = TextureRepo::new::<_, TexLoadQueue>(
            context,
            TextureLoadConfig {
                resolver: self.tex_resolver,
                filter: Filter::Linear,
                wrap_mode: WrapMode::Tile,
            },
        )
        .context("Error createing texture repo")?;

        let draw_buffers =
            DrawBuffers::from_context(context).context("Error creating draw buffers")?;
        let (pipeline, framebuffers) = {
            let mut device = context.device().write().map_err(|_| LockPoisoned::Device)?;
            let pipeline = spec
                .build(
                    &mut device,
                    context.target_chain().properties().extent,
                    context.target_chain().properties(),
                    once(&*repo.get_ds_layout()?),
                )
                .context("Error building pipeline")?;

            let fat = context.target_chain().properties().framebuffer_attachment();
            let dat = FramebufferAttachment {
                usage: Usage::DEPTH_STENCIL_ATTACHMENT,
                format: context.target_chain().properties().depth_format,
                view_caps: ViewCapabilities::empty(),
            };
            let framebuffers = TargetSpecificResources::new(
                || unsafe {
                    Ok(device.create_framebuffer(
                        &pipeline.renderpass,
                        IntoIter::new([fat.clone(), dat.clone()]),
                        context.target_chain().properties().extent,
                    )?)
                },
                context.target_chain().properties().image_count as usize,
            )?;

            (pipeline, framebuffers)
        };
        let db_spec = ImageSpec {
            width: context.target_chain().properties().extent.width,
            height: context.target_chain().properties().extent.height,
            format: context.target_chain().properties().depth_format,
            usage: Usage::DEPTH_STENCIL_ATTACHMENT,
            resources: DEPTH_RESOURCES,
        };
        let img_count = context.target_chain().properties().image_count;
        let depth_buffers = TargetSpecificResources::new(
            || {
                BoundImageView::from_context(context, &db_spec)
                    .context("Error creating depth buffer")
            },
            img_count as usize,
        )?;

        Ok(LevelDrawPass {
            pipeline,
            repo,
            draw_buffers,
            active_camera: self.active_camera,
            _d: PhantomData,
            framebuffers,
            depth_buffers,
        })
    }

    fn find_aux_queues<'c>(
        adapter: &'c Adapter,
        queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'c QueueFamilyT, Vec<f32>)>> {
        queue_negotiator.find(adapter, &TexLoadQueue)?;

        Ok(vec![
            queue_negotiator.family_spec::<TexLoadQueue>(&adapter.queue_families, 1)?
        ])
    }
}

/// Indicates an issue with the level object being used
#[derive(Debug, Error)]
pub enum LevelError {
    #[error("Referential Integrity broken")]
    BadReference,
}
