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
    RenderingContext,
    Session,
    DrawPass, IntoDrawPass, PassPosition,
    draw_passes::{util::TargetSpecificResources},
    error::LockPoisoned,
    mem::{DataPool, DepthBufferPool, StagingPool, TexturesPool},
    queue_negotiator::QueueFamilyNegotiator,
    texture::{TextureResolver, TexLoadQueue, TextureLoadConfig, TextureRepo},
    types::*,
    components::{CameraSettings, Transform},
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
use na::{look_at_lh, perspective_lh_zo};
use shaderc::ShaderKind;
use std::{
    array::IntoIter,
    convert::TryInto,
    iter::{empty, once},
    marker::PhantomData,
    sync::{Arc, RwLock},
};
use thiserror::Error;

/// The Vertexes that go to the shader
#[derive(Debug, Clone, Copy)]
struct UvPoint(pub Vector3, pub i32, pub Vector2);

/// Draw a level
pub struct LevelDrawPass<'a, M> {
    repo: TextureRepo<TexturesPool, StagingPool>,
    active_camera: Entity,
    draw_buffers: DrawBuffers<'a, UvPoint, DataPool, StagingPool>,
    surface_resources: SurfaceDependentResources,
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
        let mut query = <(&Transform, &CameraSettings)>::query();
        let (camera_transform, camera_settings) = query
            .get(&session.world, self.active_camera)
            .context("Couldn't find camera components")?;

        let camera_vp = {
            let aspect_ratio = self.surface_resources.pipeline.render_area.w as f32
                / self.surface_resources.pipeline.render_area.h as f32;

            // Get look direction from euler angles
            let direction = euler_to_direction(&camera_transform.rotation);

            // Converts world space to camera space
            let view_matrix = look_at_lh(
                &camera_transform.position,
                &(camera_transform.position + direction),
                &Vector3::new(0.0, 1.0, 0.0),
            );

            // Converts camera space to screen space
            let projection_matrix = {
                let mut temp = perspective_lh_zo(
                    aspect_ratio,
                    camera_settings.fov,
                    camera_settings.near,
                    camera_settings.far,
                );

                // Vulkan's co-ord system is different from OpenGLs
                temp[(1, 1)] *= -1.0;

                temp
            };

            // Chain them together into a single matrix
            projection_matrix * view_matrix
        };
        let map_lock: Arc<RwLock<M>> = session.resources.get::<Arc<RwLock<M>>>().unwrap().clone();
        let map = map_lock.read().map_err(|_| LockPoisoned::Map)?;

        // Get framebuffer and depth buffer
        let fb = self.surface_resources.framebuffers.get_next();
        let db = self.surface_resources.depth_buffers.get_next();

        unsafe {
            cmd_buffer.begin_render_pass(
                &self.surface_resources.pipeline.renderpass,
                fb,
                self.surface_resources.pipeline.render_area,
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
            cmd_buffer.bind_graphics_pipeline(&self.surface_resources.pipeline.pipeline);

            // VP Matrix
            let vp = &*(camera_vp.data.as_slice() as *const [f32] as *const [u32]);

            cmd_buffer.push_graphics_constants(
                &self.surface_resources.pipeline.pipeline_layout,
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
        self.surface_resources.deactivate(context)?;
        self.repo.deactivate(context);

        Ok(())
    }

    fn handle_surface_change(
        mut self,
        _session: &Session,
        context: &mut RenderingContext,
    ) -> Result<Self> {
        let new_resources =
            SurfaceDependentResources::new::<P>(context, &*self.repo.get_ds_layout()?)?;
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
                    &*self.surface_resources.pipeline.pipeline_layout,
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
        let repo = TextureRepo::new::<_, TexLoadQueue>(
            context,
            TextureLoadConfig {
                resolver: self.tex_resolver,
                filter: Filter::Linear,
                wrap_mode: WrapMode::Tile,
            },
        )
        .context("Error creating texture repo")?;
        let draw_buffers =
            DrawBuffers::from_context(context).context("Error creating draw buffers")?;

        let surface_resources =
            SurfaceDependentResources::new::<P>(context, &*repo.get_ds_layout()?)?;

        Ok(LevelDrawPass {
            repo,
            draw_buffers,
            active_camera: self.active_camera,
            surface_resources,
            _d: PhantomData,
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

/// Indicates an issue with the level object being used
#[derive(Debug, Error)]
pub enum LevelError {
    #[error("Referential Integrity broken")]
    BadReference,
}

/// Used to store resources which depend on the surface, for convenience in handle_surface_change
struct SurfaceDependentResources {
    pub pipeline: CompletePipeline,
    pub framebuffers: TargetSpecificResources<FramebufferT>,
    pub depth_buffers: TargetSpecificResources<BoundImageView<DepthBufferPool>>,
}

impl SurfaceDependentResources {
    pub fn new<P: PassPosition>(
        context: &mut RenderingContext,
        ds_layout: &DescriptorSetLayoutT,
    ) -> Result<Self> {
        let db_spec = ImageSpec {
            width: context.properties().extent.width,
            height: context.properties().extent.height,
            format: context.properties().depth_format,
            usage: Usage::DEPTH_STENCIL_ATTACHMENT,
            resources: DEPTH_RESOURCES,
        };
        let img_count = context.properties().image_count;

        let depth_buffers = TargetSpecificResources::new(
            || {
                BoundImageView::from_context(context, &db_spec)
                    .context("Error creating depth buffer")
            },
            img_count as usize,
        )?;

        let (pipeline, framebuffers) = {
            let pipeline_spec = PipelineSpecBuilder::default()
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
                            format: Some(context.properties().color_format),
                            samples: 1,
                            ops: P::attachment_ops(),
                            stencil_ops: P::attachment_ops(),
                            layouts: P::layout_as_range(),
                        },

                        used_layout: Layout::ColorAttachmentOptimal,
                    }],
                    depth: Some(AttachmentSpec {
                        attachment: Attachment {
                            format: Some(context.properties().depth_format),
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
            let mut device = context.lock_device()?;

            let pipeline = pipeline_spec
                .build(&mut device, context.properties().extent, once(ds_layout))
                .context("Error building pipeline")?;

            let fat = context.properties().swapchain_framebuffer_attachment();
            let dat = FramebufferAttachment {
                usage: Usage::DEPTH_STENCIL_ATTACHMENT,
                format: context.properties().depth_format,
                view_caps: ViewCapabilities::empty(),
            };

            let framebuffers = TargetSpecificResources::new(
                || unsafe {
                    Ok(device.create_framebuffer(
                        &pipeline.renderpass,
                        IntoIter::new([fat.clone(), dat.clone()]),
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
            depth_buffers,
        })
    }

    pub fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
        for db in self.depth_buffers.dissolve() {
            db.deactivate_with_context(context);
        }
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

fn euler_to_direction(euler: &Vector3) -> Vector3 {
    let pitch = euler.x;
    let yaw = euler.y;
    let _roll = euler.z; // TODO: Support camera roll

    Vector3::new(
        yaw.sin() * pitch.cos(),
        pitch.sin(),
        yaw.cos() * pitch.cos(),
    )
}
