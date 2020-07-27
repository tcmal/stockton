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
//! In the end, this takes in vertices and renders them to a window.
//! You'll need something else to actually generate the vertices though.

use std::{
	mem::{ManuallyDrop, size_of},
	ops::Deref,
	borrow::Borrow,
	convert::TryInto
};
use winit::window::Window;
use arrayvec::ArrayVec;

use hal::{
	prelude::*,
	queue::{Submission},
	window::SwapchainConfig
};
use stockton_types::{Vector2, Vector3};
use stockton_levels::prelude::*;
use stockton_levels::traits::faces::FaceType;

use crate::{
	types::*,
	error
};
use super::{
	camera::WorkingCamera,
	texture::TextureStore,
	buffer::{StagedBuffer, ModifiableBuffer}
};

/// Entry point name for shaders
const ENTRY_NAME: &str = "main";

/// Defines the colour range we use.
const COLOR_RANGE: hal::image::SubresourceRange = hal::image::SubresourceRange {
	aspects: hal::format::Aspects::COLOR,
	levels: 0..1,
	layers: 0..1,
};

/// Initial size of vertex buffer. TODO: Way of overriding this
const INITIAL_VERT_SIZE: u64 = 3 * 3000;

/// Initial size of index buffer. TODO: Way of overriding this
const INITIAL_INDEX_SIZE: u64 = 3000;

/// Source for vertex shader. TODO
const VERTEX_SOURCE: &str = include_str!("./data/stockton.vert");

/// Source for fragment shader. TODO
const FRAGMENT_SOURCE: &str = include_str!("./data/stockton.frag");

/// Represents a point of a triangle, including UV and texture information.
#[derive(Debug, Clone, Copy)]
pub struct UVPoint (pub Vector3, pub i32, pub Vector2);

/// Contains all the hal related stuff.
/// In the end, this takes some 3D points and puts it on the screen.
// TODO: Settings for clear colour, buffer sizes, etc
pub struct RenderingContext<'a> {
	// Parents for most of these things
	instance: ManuallyDrop<back::Instance>,
	device: ManuallyDrop<Device>,
	adapter: Adapter,

	// Render destination
	surface: ManuallyDrop<Surface>,
	swapchain: ManuallyDrop<Swapchain>,
	viewport: hal::pso::Viewport,
	
	imageviews: Vec<ImageView>,
	framebuffers: Vec<Framebuffer>,
	current_frame: usize,
	frames_in_flight: usize,
	
	// Sync objects
	// TODO: Collect these together?
	get_image: Vec<Semaphore>,
	render_complete: Vec<Semaphore>,
	present_complete: Vec<Fence>,

	// Pipeline
	renderpass: ManuallyDrop<RenderPass>,
	pipeline_layout: ManuallyDrop<PipelineLayout>,
	pipeline: ManuallyDrop<GraphicsPipeline>,

	// Command pool and buffers
	cmd_pool: ManuallyDrop<CommandPool>,
	cmd_buffers: Vec<CommandBuffer>,
	queue_group: QueueGroup,

	// Texture store
	texture_store: ManuallyDrop<TextureStore>,

	// Vertex and index buffers
	// These are both staged
	pub vert_buffer: ManuallyDrop<StagedBuffer<'a, UVPoint>>,
	pub index_buffer: ManuallyDrop<StagedBuffer<'a, (u16, u16, u16)>>,

	camera: WorkingCamera
}

impl<'a> RenderingContext<'a> {
	/// Create a new RenderingContext for the given window.
	pub fn new<T: HasTextures>(window: &Window, file: &T) -> Result<Self, error::CreationError> {
		// Create surface
		let (instance, mut surface, mut adapters) = unsafe {
			use hal::Instance;

			let instance = back::Instance::create("stockton", 1).map_err(|_| error::CreationError::WindowError)?;
			let surface = instance.create_surface(window).map_err(|_| error::CreationError::WindowError)?;
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

		// Swapchain
		let (format, viewport, extent, swapchain, backbuffer) = RenderingContext::create_swapchain(&mut surface, &mut device, &adapter, None)?;

		// Renderpass
		let renderpass = {
			use hal::{
				pass::*,
				pso::PipelineStage,
				image::{Access, Layout},
				memory::Dependencies
			};

			let attachment = Attachment {
				format: Some(format),
				samples: 1,
				ops: AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
				stencil_ops: AttachmentOps::new(AttachmentLoadOp::DontCare, AttachmentStoreOp::DontCare),
				layouts: Layout::Undefined..Layout::Present
			};

			let subpass = SubpassDesc {
				colors: &[(0, Layout::ColorAttachmentOptimal)],
				depth_stencil: None,
				inputs: &[],
				resolves: &[],
				preserves: &[]
			};

			let dependency = SubpassDependency {
				flags: Dependencies::empty(),
				passes: None..Some(0),
				stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
				accesses: Access::empty()
					..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE)
			};

			unsafe { device.create_render_pass(&[attachment], &[subpass], &[dependency]) }
				.map_err(|_| error::CreationError::OutOfMemoryError)?
		};

		// Subpass
		let subpass = hal::pass::Subpass {
			index: 0,
			main_pass: &renderpass
		};

		// Vertex and index buffers
		let (vert_buffer, index_buffer) = {
			use hal::buffer::Usage;

			let vert = StagedBuffer::new(&mut device, &adapter, Usage::VERTEX, INITIAL_VERT_SIZE)?;
			let index = StagedBuffer::new(&mut device, &adapter, Usage::INDEX, INITIAL_INDEX_SIZE)?;
			
			(vert, index)
		};

		// Command Pool, Buffers, imageviews, framebuffers & Sync objects
		let frames_in_flight = backbuffer.len();
		let (mut cmd_pool, cmd_buffers, get_image, render_complete, present_complete, imageviews, framebuffers) = {
			use hal::pool::CommandPoolCreateFlags;
			use hal::command::Level;

			let mut cmd_pool = ManuallyDrop::new(unsafe {
				device.create_command_pool(queue_group.family, CommandPoolCreateFlags::RESET_INDIVIDUAL)
			}.map_err(|_| error::CreationError::OutOfMemoryError)?);

			let mut cmd_buffers = Vec::with_capacity(frames_in_flight);
			let mut get_image = Vec::with_capacity(frames_in_flight);
			let mut render_complete = Vec::with_capacity(frames_in_flight);
			let mut present_complete = Vec::with_capacity(frames_in_flight);
			let mut imageviews = Vec::with_capacity(frames_in_flight);
			let mut framebuffers = Vec::with_capacity(frames_in_flight);

			for i in 0..frames_in_flight {
				unsafe {
					cmd_buffers.push(cmd_pool.allocate_one(Level::Primary)); // TODO: We can do this all at once outside the loop
				}

				get_image.push(device.create_semaphore().map_err(|_| error::CreationError::SyncObjectError)?);
				render_complete.push(device.create_semaphore().map_err(|_| error::CreationError::SyncObjectError)?);
				present_complete.push(device.create_fence(true).map_err(|_| error::CreationError::SyncObjectError)?);
				
				unsafe {
					use hal::image::ViewKind;
					use hal::format::Swizzle;

					imageviews.push(device.create_image_view(
						&backbuffer[i],
						ViewKind::D2,
						format,
						Swizzle::NO,
						COLOR_RANGE.clone(),
					).map_err(|e| error::CreationError::ImageViewError (e))?);
					framebuffers.push(device.create_framebuffer(
						&renderpass,
						Some(&imageviews[i]),
						extent
					).map_err(|_| error::CreationError::OutOfMemoryError)?);
				}
			}

			(cmd_pool, cmd_buffers, get_image, render_complete, present_complete, imageviews, framebuffers)
		};

		// Texture store
		let texture_store = TextureStore::new(&mut device,
			&mut adapter,
			&mut queue_group.queues[0],
			&mut cmd_pool, file)?;

		// Camera
		// TODO: Settings
		let ratio = extent.width as f32 / extent.height as f32;
		let camera = WorkingCamera::defaults(ratio);

		let mut descriptor_set_layouts: ArrayVec<[_; 2]> = ArrayVec::new();
		descriptor_set_layouts.push(texture_store.descriptor_set_layout.deref());

		// Graphics pipeline
		let (pipeline_layout, pipeline) = Self::create_pipeline(&mut device, extent, &subpass, descriptor_set_layouts)?;

		Ok(RenderingContext {
			instance: ManuallyDrop::new(instance),
			surface: ManuallyDrop::new(surface),

			device: ManuallyDrop::new(device),
			adapter,
			queue_group,
			swapchain: ManuallyDrop::new(swapchain),
			viewport,

			imageviews,
			framebuffers,

			renderpass: ManuallyDrop::new(renderpass),
			current_frame: 0,

			get_image,
			render_complete,
			present_complete,
			frames_in_flight,
			cmd_pool,
			cmd_buffers,

			pipeline_layout: ManuallyDrop::new(pipeline_layout),
			pipeline: ManuallyDrop::new(pipeline),

			texture_store: ManuallyDrop::new(texture_store),

			vert_buffer: ManuallyDrop::new(vert_buffer),
			index_buffer: ManuallyDrop::new(index_buffer),

			camera
		})
	}

	/// If this function fails the whole context is probably dead
	/// The context must not be used while this is being called
	pub unsafe fn handle_surface_change(&mut self) -> Result<(), error::CreationError> {
		use core::ptr::read;

		self.device.wait_idle().unwrap();

		// Swapchain itself
		let old_swapchain = ManuallyDrop::into_inner(read(&self.swapchain));

		let (format, viewport, extent, swapchain, backbuffer) = RenderingContext::create_swapchain(&mut self.surface, &mut self.device, &self.adapter, Some(old_swapchain))?;

		self.swapchain = ManuallyDrop::new(swapchain);
		self.viewport = viewport;

		// Camera settings (aspect ratio)
		self.camera.update_aspect_ratio(extent.width as f32 / extent.height as f32);

		// Graphics pipeline
		self.device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));
			
		self.device
			.destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));
	

		let (pipeline_layout, pipeline) = {		
			let mut descriptor_set_layouts: ArrayVec<[_; 2]> = ArrayVec::new();
			descriptor_set_layouts.push(self.texture_store.descriptor_set_layout.deref());

			let subpass = hal::pass::Subpass {
				index: 0,
				main_pass: &(*self.renderpass)
			};

			Self::create_pipeline(&mut self.device, extent, &subpass, descriptor_set_layouts)?
		};

		self.pipeline_layout = ManuallyDrop::new(pipeline_layout);
		self.pipeline = ManuallyDrop::new(pipeline);

		// Imageviews, framebuffers
		// Destroy old objects
		for framebuffer in self.framebuffers.drain(..) {
			self.device.destroy_framebuffer(framebuffer);
		}
		for image_view in self.imageviews.drain(..) {
			self.device.destroy_image_view(image_view);
		}
		// Make new ones
		for i in 0..backbuffer.len() {
			use hal::image::ViewKind;
			use hal::format::Swizzle;

			self.imageviews.push(self.device.create_image_view(
				&backbuffer[i],
				ViewKind::D2,
				format,
				Swizzle::NO,
				COLOR_RANGE.clone(),
			).map_err(|e| error::CreationError::ImageViewError (e))?);

			self.framebuffers.push(self.device.create_framebuffer(
				&self.renderpass,
				Some(&self.imageviews[i]),
				extent
			).map_err(|_| error::CreationError::OutOfMemoryError)?);
		}

		Ok(())
	} 

	fn create_swapchain(surface: &mut Surface, device: &mut Device, adapter: &Adapter, old_swapchain: Option<Swapchain>) -> Result<(
			hal::format::Format,
			hal::pso::Viewport,
			hal::image::Extent,
			Swapchain,
			Vec<Image>
		), error::CreationError> {
		use hal::{
			window::{PresentMode, CompositeAlphaMode},
			format::{Format, ChannelType},
			image::Usage,
			pso::Viewport
		};

		// Figure out what the surface supports
		let caps = surface.capabilities(&adapter.physical_device);
		let formats = surface.supported_formats(&adapter.physical_device);

		// Find which settings we'll actually use based on preset preferences
		let format = formats.map_or(Format::Rgba8Srgb, |formats| {
			formats.iter()
				.find(|format| format.base_format().1 == ChannelType::Srgb)
				.map(|format| *format)
				.unwrap_or(formats[0])
		});

		let present_mode = {
			[PresentMode::MAILBOX, PresentMode::FIFO, PresentMode::RELAXED, PresentMode::IMMEDIATE]
				.iter()
				.cloned()
				.find(|pm| caps.present_modes.contains(*pm))
				.ok_or(error::CreationError::BadSurface)?
		};
		let composite_alpha = {
			[CompositeAlphaMode::OPAQUE, CompositeAlphaMode::INHERIT, CompositeAlphaMode::PREMULTIPLIED, CompositeAlphaMode::POSTMULTIPLIED]
				.iter()
				.cloned()
				.find(|ca| caps.composite_alpha_modes.contains(*ca))
				.ok_or(error::CreationError::BadSurface)?
		};

		// Figure out properties for our swapchain
		let extent = caps.extents.end(); // Size

		// Number of frames to pre-render
		let image_count = if present_mode == PresentMode::MAILBOX {
			((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(3))
		} else {
			((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(2))
		};

		let image_layers = 1; // Don't support 3D
		let image_usage = if caps.usage.contains(Usage::COLOR_ATTACHMENT) {
			Usage::COLOR_ATTACHMENT
		} else {
			Err(error::CreationError::BadSurface)?
		};

		// Swap config
		let swap_config = SwapchainConfig {
			present_mode,
			composite_alpha_mode: composite_alpha,
			format,
			extent: *extent,
			image_count,
			image_layers,
			image_usage,
		};

		// Viewport
		let extent = extent.to_extent();
		let viewport = Viewport {
			rect: extent.rect(),
			depth: 0.0..1.0
		};
		
		// Swapchain
		let (swapchain, backbuffer) = unsafe {
			device.create_swapchain(surface, swap_config, old_swapchain)
				.map_err(|e| error::CreationError::SwapchainError (e))?
		};

		Ok((format, viewport, extent, swapchain, backbuffer))
	}

	#[allow(clippy::type_complexity)]
	fn create_pipeline<T>(device: &mut Device, extent: hal::image::Extent, subpass: &hal::pass::Subpass<back::Backend>, set_layouts: T) -> Result<
	(
	  PipelineLayout,
	  GraphicsPipeline,
	), error::CreationError> where T: IntoIterator, T::Item: Borrow<DescriptorSetLayout> {
		use hal::pso::*;
		use hal::format::Format;

		// Shader modules
		let (vs_module, fs_module) = {
			let mut compiler = shaderc::Compiler::new().ok_or(error::CreationError::NoShaderC)?;

			let vertex_compile_artifact = compiler
				.compile_into_spirv(VERTEX_SOURCE, shaderc::ShaderKind::Vertex, "vertex.vert", ENTRY_NAME, None)
				.map_err(|e| error::CreationError::ShaderCError (e))?;
			
			let fragment_compile_artifact = compiler
				.compile_into_spirv(FRAGMENT_SOURCE, shaderc::ShaderKind::Fragment, "fragment.frag", ENTRY_NAME, None)
				.map_err(|e| error::CreationError::ShaderCError (e))?;
			
			// Make into shader module
			unsafe {
				(device
					.create_shader_module(vertex_compile_artifact.as_binary())
					.map_err(|e| error::CreationError::ShaderModuleFailed (e))?,
				device
					.create_shader_module(fragment_compile_artifact.as_binary())
					.map_err(|e| error::CreationError::ShaderModuleFailed (e))?)
			}
		};

		// Shader entry points (ShaderStage)
		let (vs_entry, fs_entry) = (
			EntryPoint::<back::Backend> {
				entry: ENTRY_NAME,
				module: &vs_module,
				specialization: Specialization::default()
			},
			EntryPoint::<back::Backend> {
				entry: ENTRY_NAME,
				module: &fs_module,
				specialization: Specialization::default()
			}
		);

		// Shader set
		let shaders = GraphicsShaderSet {
			vertex: vs_entry,
			fragment: Some(fs_entry),
			hull: None,
			domain: None,
			geometry: None
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
			line_width: hal::pso::State::Static(1.0)
		};

		// Depth stencil
		let depth_stencil = DepthStencilDesc {
			depth: None,
			depth_bounds: false,
			stencil: None,
		};

		// Pipeline layout
		let layout = unsafe {
			device.create_pipeline_layout(
				set_layouts,
				// vp matrix, 4x4 f32
				&[(ShaderStageFlags::VERTEX, 0..64)]
			)
		}.map_err(|_| error::CreationError::OutOfMemoryError)?;

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
				targets: vec![ColorBlendDesc { mask: ColorMask::ALL, blend: Some(blend_state) }],
			}
		};

		// Baked states
		let baked_states = BakedStates {
			viewport: Some(Viewport {
				rect: extent.rect(),
				depth: (0.0..1.0)
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
			attributes
		};

		// Pipeline
		let pipeline = unsafe {
			device.create_graphics_pipeline(&pipeline_desc, None)
		}.map_err(|e| error::CreationError::PipelineError (e))?;

		Ok((layout, pipeline))
	}

	/// Draw all vertices in the buffer
	pub fn draw_vertices(&mut self) -> Result<(), &'static str> {
		let get_image = &self.get_image[self.current_frame];
		let render_complete = &self.render_complete[self.current_frame];
		
		// Advance the frame _before_ we start using the `?` operator
		self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

		// Get the image
		let (image_index, _) = unsafe {
			self
				.swapchain
				.acquire_image(core::u64::MAX, Some(get_image), None)
				.map_err(|_| "FrameError::AcquireError")?
		};
		let image_index = image_index as usize;

		// Make sure whatever was last using this has finished
		let present_complete = &self.present_complete[image_index];
		unsafe {
			self.device
				.wait_for_fence(present_complete, core::u64::MAX)
				.map_err(|_| "FrameError::SyncObjectError")?;
			self.device
				.reset_fence(present_complete)
				.map_err(|_| "FrameError::SyncObjectError")?;
		};

		// Record commands
		unsafe {
			use hal::buffer::{IndexBufferView, SubRange};
			use hal::command::{SubpassContents, CommandBufferFlags, ClearValue, ClearColor};
			use hal::pso::ShaderStageFlags;

			let buffer = &mut self.cmd_buffers[image_index];
			let clear_values = [ClearValue {
				color: ClearColor {
					float32: [0.0, 0.0, 0.0, 1.0]
				}
			}];

			// Commit from staging buffers
			let (vbufs, ibuf) = {
				let vbufref: &<back::Backend as hal::Backend>::Buffer = self.vert_buffer.commit(
					&self.device,
					&mut self.queue_group.queues[0],
					&mut self.cmd_pool
				);

				let vbufs: ArrayVec<[_; 1]> = [(vbufref, SubRange::WHOLE)].into();
				let ibuf = self.index_buffer.commit(
					&self.device,
					&mut self.queue_group.queues[0],
					&mut self.cmd_pool
				);

				(vbufs, ibuf)
			};

			buffer.begin_primary(CommandBufferFlags::EMPTY);
			{
				buffer.begin_render_pass(
					&self.renderpass,
					&self.framebuffers[image_index],
					self.viewport.rect,
					clear_values.iter(),
					SubpassContents::Inline
				);
				buffer.bind_graphics_pipeline(&self.pipeline);

				let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
				descriptor_sets.push(self.texture_store.get_chunk_descriptor_set(0));

				buffer.bind_graphics_descriptor_sets(
					&self.pipeline_layout,
					0,
					descriptor_sets,
					&[]
				);

				let vp = self.camera.get_matrix().as_slice();
				let vp = std::mem::transmute::<&[f32], &[u32]>(vp);

				buffer.push_graphics_constants(
					&self.pipeline_layout,
					ShaderStageFlags::VERTEX,
					0,
					vp);

				buffer.bind_vertex_buffers(0, vbufs);
				buffer.bind_index_buffer(IndexBufferView {
					buffer: ibuf,
					range: SubRange::WHOLE,
					index_type: hal::IndexType::U16
				});
				buffer.draw_indexed(0..((self.index_buffer.highest_used as u32 + 1) * 3), 0, 0..1);
				buffer.end_render_pass();
			}
			buffer.finish();
		};

		// Make submission object
		let command_buffers = &self.cmd_buffers[image_index..=image_index];
		let wait_semaphores: ArrayVec<[_; 1]> = [(get_image, hal::pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
		let signal_semaphores: ArrayVec<[_; 1]> = [render_complete].into();

		let present_wait_semaphores: ArrayVec<[_; 1]> = [render_complete].into();

		let submission = Submission {
			command_buffers,
			wait_semaphores,
			signal_semaphores,
		};

		// Submit it
		let command_queue = &mut self.queue_group.queues[0];
		unsafe {
			command_queue.submit(submission, Some(present_complete));
			self.swapchain
				.present(command_queue, image_index as u32, present_wait_semaphores)
				.map_err(|_| "FrameError::PresentError")?
		};

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

	/// Load all active faces into the vertex buffers for drawing
	// TODO: This is just a POC, we need to restructure things a lot for actually texturing, etc
	pub fn set_active_faces<M: MinBSPFeatures<VulkanSystem>>(&mut self, faces: &Vec<u32>, file: &M) -> () {
		let mut curr_vert_idx: usize = 0;
		let mut curr_idx_idx: usize = 0;

		for face in faces.into_iter().map(|idx| file.get_face(*idx)) {
			if face.face_type == FaceType::Polygon || face.face_type == FaceType::Mesh {
				let base = face.vertices_idx.start;

				for idx in face.meshverts_idx.clone().step_by(3) {
					let start_idx: u16 = curr_vert_idx.try_into().unwrap();

					for idx2 in idx..idx+3 {
						let vert = &file.resolve_meshvert(idx2 as u32, base);
						let uv = Vector2::new(vert.tex.u[0], vert.tex.v[0]);

						let uvp = UVPoint (vert.position, face.texture_idx.try_into().unwrap(), uv);
						self.vert_buffer[curr_vert_idx] = uvp;
						
						curr_vert_idx += 1;
					}


					self.index_buffer[curr_idx_idx] = (start_idx, start_idx + 1, start_idx + 2);

					curr_idx_idx += 1;

					if curr_vert_idx >= INITIAL_VERT_SIZE.try_into().unwrap() || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into().unwrap() {
						break;
					}
				}
			} else {
				// TODO: Other types of faces
			}

			if curr_vert_idx >= INITIAL_VERT_SIZE.try_into().unwrap() || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into().unwrap() {
				break;
			}
		}
	}
}

impl<'a> core::ops::Drop for RenderingContext<'a> {
	fn drop(&mut self) {
		// TODO: Probably missing some destroy stuff
		self.device.wait_idle().unwrap();

		unsafe {
			for fence in self.present_complete.drain(..) {
				self.device.destroy_fence(fence)
			}
			for semaphore in self.render_complete.drain(..) {
				self.device.destroy_semaphore(semaphore)
			}
			for semaphore in self.get_image.drain(..) {
				self.device.destroy_semaphore(semaphore)
			}
			for framebuffer in self.framebuffers.drain(..) {
				self.device.destroy_framebuffer(framebuffer);
			}
			for image_view in self.imageviews.drain(..) {
				self.device.destroy_image_view(image_view);
			}

			use core::ptr::read;
			ManuallyDrop::into_inner(read(&self.vert_buffer)).deactivate(&mut self.device);
			ManuallyDrop::into_inner(read(&self.index_buffer)).deactivate(&mut self.device);
			ManuallyDrop::into_inner(read(&self.texture_store)).deactivate(&mut self.device);

			self.device.destroy_command_pool(
				ManuallyDrop::into_inner(read(&self.cmd_pool)),
			);

			self.device
				.destroy_render_pass(ManuallyDrop::into_inner(read(&self.renderpass)));
			self.device
				.destroy_swapchain(ManuallyDrop::into_inner(read(&self.swapchain)));

			self.device.destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));
			
			self.device
				.destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));

			self.instance
				.destroy_surface(ManuallyDrop::into_inner(read(&self.surface)));

			ManuallyDrop::drop(&mut self.device);
		}
	}
}