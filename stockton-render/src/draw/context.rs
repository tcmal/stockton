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
use core::mem::{ManuallyDrop, size_of};
use std::borrow::Cow;

use crate::error::{CreationError, FrameError};
use super::frame::FrameCell;

use arrayvec::ArrayVec;

use winit::Window;

// Trait imports
use hal::{Surface as SurfaceTrait, Instance as InstanceTrait, QueueFamily as QFTrait, PhysicalDevice as PDTrait, Device as DeviceTrait, Swapchain as SwapchainTrait};

use hal::{Graphics, Gpu, Features, SwapchainConfig, Submission};
use hal::pso::*;
use hal::pass::{Subpass, SubpassDesc, AttachmentOps, Attachment, AttachmentStoreOp, AttachmentLoadOp};
use hal::image::{Usage, Layout, SubresourceRange, ViewKind, Extent};
use hal::command::{ClearValue, ClearColor, CommandBuffer};
use hal::format::{ChannelType, Format, Swizzle, Aspects};
use hal::pool::{CommandPoolCreateFlags, CommandPool};
use hal::window::{PresentMode, Extent2D};
use hal::queue::family::QueueGroup;
use hal::adapter::{Adapter, MemoryTypeId};

use back::{Instance};
use back::{Backend};

use stockton_types::Vector2;

const VERTEX_SOURCE: &str = "#version 450
layout (location = 0) in vec2 position;
out gl_PerVertex {
	vec4 gl_Position;
};
void main()
{
  gl_Position = vec4(position, 0.0, 1.0);
}";
const FRAGMENT_SOURCE: &str = "#version 450
layout(location = 0) out vec4 color;
void main()
{
  color = vec4(1.0);
}";

/// Represents a triangle in 2D (screen) space.
pub struct Tri2 (pub [Vector2; 3]);

/// Easy conversion to proper format.
impl From<Tri2> for [f32; 6] {
	fn from(tri: Tri2) -> [f32; 6] {
	    [tri.0[0].x, tri.0[0].y,
	     tri.0[1].x, tri.0[1].y,
	     tri.0[2].x, tri.0[2].y]
	}
}

/// Size of Tri2
const F32_XY_TRIANGLE: u64 = (size_of::<f32>() * 2 * 3) as u64;

/// Contains all the hal related stuff.
/// In the end, this takes some 3D points and puts it on the screen.
pub struct RenderingContext<'a> {
	/// The window to render to
	window: &'a Window,

	/// The instance of vulkan (or another backend) being used.
	/// For vulkan, the name is always "stockton" & version is always 1
	instance: ManuallyDrop<Instance>,

	/// The surface being drawn to.
	/// Mostly an abstraction over Window.
	surface: <Backend as hal::Backend>::Surface,
	
	/// Points to something that's used for rendering & presentation.
	adapter: Adapter<Backend>,

	/// Lets us actually render & present
	device: ManuallyDrop<<Backend as hal::Backend>::Device>,
	
	/// A collection of images that we create then present.
	swapchain: ManuallyDrop<<Backend as hal::Backend>::Swapchain>,

	/// The size of where we're rendering to.
	render_area: Rect,

	/// Describes the render pipeline.
	render_pass: ManuallyDrop<<Backend as hal::Backend>::RenderPass>,

	/// The format of the images.
	format: Format,

	/// What part(s) of the device will do our work
	queue_group: ManuallyDrop<QueueGroup<Backend, Graphics>>,

	/// A parent for all our command buffers.
	command_pool: ManuallyDrop<CommandPool<Backend, Graphics>>,

	/// Each FrameCell draws a frame to its own image & command buffer.
	cells: Vec<FrameCell>,

	/// Images from the swapchain to render to.
	backbuffer: Vec<<Backend as hal::Backend>::Image>,

	/// The images we're rendering to with some additional metadata.
	image_views: Vec<<Backend as hal::Backend>::ImageView>,

	/// What the command buffers will  actually target.
	framebuffers: Vec<<Backend as hal::Backend>::Framebuffer>,

	/// The maximum number of frames we'll have ready for presentation at any time.
	frames_in_flight: usize,

	/// Track which framecell is up next.
	current_frame: usize,

	descriptor_set_layouts: Vec<<Backend as hal::Backend>::DescriptorSetLayout>,

	pipeline_layout: ManuallyDrop<<Backend as hal::Backend>::PipelineLayout>,

	pipeline: ManuallyDrop<<Backend as hal::Backend>::GraphicsPipeline>,

	buffer: ManuallyDrop<<Backend as hal::Backend>::Buffer>,
	memory: ManuallyDrop<<Backend as hal::Backend>::Memory>,
	requirements: hal::memory::Requirements
}

impl<'a> RenderingContext<'a> {
	/// Create a new RenderingContext for the given window.
	pub fn new(window: &'a Window) -> Result<Self, CreationError> {
		let instance = Instance::create("stockton", 1);
		let mut surface = instance.create_surface(&window);

		// find a suitable adapter
		// one that can render graphics & present to our surface
		let adapter = instance
		  .enumerate_adapters()
		  .into_iter()
		  .find(|a| {
			a.queue_families
			  .iter()
			  .any(|qf| qf.supports_graphics() && surface.supports_queue_family(qf))
		  })
		  .ok_or(CreationError::NoAdapter)?;

		// from that adapter, get the device & queue group
		let (mut device, queue_group) = {
			let queue_family = adapter
				.queue_families
				.iter()
				.find(|qf| qf.supports_graphics() && surface.supports_queue_family(qf))
				.ok_or(CreationError::NoQueueFamily)?;
			
			let Gpu { device, mut queues } = unsafe {
				adapter
					.physical_device
					.open(&[(&queue_family, &[1.0; 1])], Features::empty())
					.map_err(|_| CreationError::NoPhysicalDevice)?
			};

			let queue_group = queues
				.take::<Graphics>(queue_family.id())
				.ok_or(CreationError::NoQueueGroup)?;

			if queue_group.queues.is_empty() {
				return Err(CreationError::NoCommandQueues)
			};
			
			(device, queue_group)
		};

		// Create the swapchain
		let (swapchain, extent, backbuffer, format, frames_in_flight) = {
			let (caps, formats, modes) = surface.compatibility(&adapter.physical_device);

			let present_mode = {
				use hal::window::PresentMode::*;
				[Mailbox, Fifo, Relaxed, Immediate]
					.iter()
					.cloned()
					.find(|pm| modes.contains(pm))
					.ok_or(CreationError::NoPresentModes)?
			};

			let composite_alpha = {
				use hal::window::CompositeAlpha;
				[CompositeAlpha::OPAQUE, CompositeAlpha::INHERIT, CompositeAlpha::PREMULTIPLIED, CompositeAlpha::POSTMULTIPLIED]
					.iter()
					.cloned()
					.find(|ca| caps.composite_alpha.contains(*ca))
					.ok_or(CreationError::NoCompositeAlphas)?
			};

			let format = match formats {
				None => Format::Rgba8Srgb,
				Some(formats) => match formats
									.iter()
									.find(|format| format.base_format().1 == ChannelType::Srgb)
									.cloned() {
					Some(srgb_format) => srgb_format,
					None => formats.get(0).cloned().ok_or(CreationError::NoImageFormats)?,
				},
			};

			let extent = {
				let window_client_area = window
					.get_inner_size()
					.ok_or(CreationError::NoWindow)?
					.to_physical(window.get_hidpi_factor());
				Extent2D {
					width: caps.extents.end.width.min(window_client_area.width as u32),
					height: caps.extents.end.height.min(window_client_area.height as u32)
				}
			};
			let image_count = if present_mode == PresentMode::Mailbox {
				(caps.image_count.end - 1).min(3)
			} else {
				(caps.image_count.end - 1).min(2)
			};

			let image_layers = 1;
			let image_usage = if caps.usage.contains(Usage::COLOR_ATTACHMENT) {
				Usage::COLOR_ATTACHMENT
			} else {
				Err(CreationError::NoColor)?
			};


			let swapchain_config = SwapchainConfig {
				present_mode,
				composite_alpha,
				format,
				extent,
				image_count,
				image_layers,
				image_usage,
			};
			let (swapchain, backbuffer) = unsafe {
				device
				  .create_swapchain(&mut surface, swapchain_config, None)
				  .map_err(|e| CreationError::SwapchainFailed { 0: e})?
			};
			
			(swapchain, extent, backbuffer, format, image_count as usize)
		};

		let render_area = Rect {
			x: 0, y: 0,
			w: extent.width as i16,
			h: extent.height as i16
		};

		// Create render pass
		let render_pass = {
			let color_attachment = Attachment {
				format: Some(format),
				samples: 1,
				ops: AttachmentOps {
					load: AttachmentLoadOp::Clear,
					store: AttachmentStoreOp::Store,
				},
				stencil_ops: AttachmentOps::DONT_CARE,
				layouts: Layout::Undefined..Layout::Present,
			};
			let subpass = SubpassDesc {
				colors: &[(0, Layout::ColorAttachmentOptimal)],
				depth_stencil: None,
				inputs: &[],
				resolves: &[],
				preserves: &[],
			};
			unsafe {
				device
				.create_render_pass(&[color_attachment], &[subpass], &[])
				.map_err(|e| CreationError::RenderPassFailed { 0: e })?
			}
		};

		// Graphics pipeline
		let (descriptor_set_layouts, pipeline_layout, pipeline) = RenderingContext::create_pipeline(&mut device, extent, &render_pass)?;

		// Vertex Buffer
		let (mut buffer, memory, requirements) = unsafe {
			let buffer = device
				.create_buffer(F32_XY_TRIANGLE, hal::buffer::Usage::VERTEX)
				.map_err(|e| CreationError::BufferFailed (e))?;

			let requirements = device.get_buffer_requirements(&buffer);
			let memory_type_id = adapter.physical_device
				.memory_properties().memory_types
				.iter().enumerate()
				.find(|&(id, memory_type)| {
					requirements.type_mask & (1 << id) != 0 && memory_type.properties.contains(hal::memory::Properties::CPU_VISIBLE)
				})
				.map(|(id, _)| MemoryTypeId(id))
				.ok_or(CreationError::NoMemory)?;

			let memory = device
				.allocate_memory(memory_type_id, requirements.size)
				.map_err(|e| CreationError::AllocationFailed (e))?;

			(buffer, memory, requirements)
		};


		// Make the command pool
		let mut command_pool = unsafe {
			device
				.create_command_pool_typed(&queue_group, CommandPoolCreateFlags::RESET_INDIVIDUAL)
				.map_err(|e| CreationError::CommandPoolFailed { 0: e })?
		};

		// Create framecells
		let cells = (0..frames_in_flight)
			.map(|_| {
				let image_available = device.create_semaphore().map_err(|e| CreationError::SemaphoreFailed { 0: e })?;
				let render_finished = device.create_semaphore().map_err(|e| CreationError::SemaphoreFailed { 0: e })?;
				let frame_presented = device.create_fence(true).map_err(|e| CreationError::FenceFailed { 0: e })?;


				let command_buffer = command_pool.acquire_command_buffer();

				Ok(FrameCell {
					command_buffer,
					image_available,
					render_finished,
					frame_presented
				})
			})
			.collect::<Result<Vec<FrameCell>, CreationError>>()?;

		// Create image views and framebuffers
		let image_views = backbuffer.iter().map(
			|image| unsafe {
				device.create_image_view(
					&image,
					ViewKind::D2,
					format,
					Swizzle::NO,
					SubresourceRange {
						aspects: Aspects::COLOR,
						levels: 0..1,
						layers: 0..1,
					},
				).map_err(|e| CreationError::ImageViewFailed { 0: e })
			}
		).collect::<Result<Vec<<Backend as hal::Backend>::ImageView>, CreationError>>()?;
		
		let framebuffers = image_views.iter().map(
			|image_view| unsafe {
				device.create_framebuffer(
					&render_pass,
					vec![image_view],
					Extent {
						width: extent.width as u32,
						height: extent.height as u32,
						depth: 1
					}
				).map_err(|e| CreationError::FramebufferFailed { 0: e })
			}
		).collect::<Result<Vec<<Backend as hal::Backend>::Framebuffer>, CreationError>>()?;


		Ok(RenderingContext {
			instance: ManuallyDrop::new(instance),
			
			window, surface,

			device: ManuallyDrop::new(device),
			adapter,
			queue_group: ManuallyDrop::new(queue_group),
			
			swapchain: ManuallyDrop::new(swapchain), 
			render_area, format, frames_in_flight,

			framebuffers, image_views, backbuffer,
			
			render_pass: ManuallyDrop::new(render_pass),
			cells,
		
			command_pool: ManuallyDrop::new(command_pool), 
	
			current_frame: 0,

			descriptor_set_layouts,
			pipeline_layout: ManuallyDrop::new(pipeline_layout),
			pipeline: ManuallyDrop::new(pipeline),

			buffer: ManuallyDrop::new(buffer),
			memory: ManuallyDrop::new(memory),
			requirements
		})
	}

	#[allow(clippy::type_complexity)]
	fn create_pipeline(device: &mut back::Device, extent: Extent2D, render_pass: &<Backend as hal::Backend>::RenderPass)
		-> Result<(
	    	Vec<<Backend as hal::Backend>::DescriptorSetLayout>,
	    	<Backend as hal::Backend>::PipelineLayout,
	    	<Backend as hal::Backend>::GraphicsPipeline,
	    ), CreationError> {

		// Compile shaders
		let mut compiler = shaderc::Compiler::new().ok_or(CreationError::NoShaderC)?;

		let vertex_compile_artifact = compiler
			.compile_into_spirv(VERTEX_SOURCE, shaderc::ShaderKind::Vertex, "vertex.vert", "main", None)
			.map_err(|e| CreationError::ShaderCError (e))?;
		
		let fragment_compile_artifact = compiler
			.compile_into_spirv(FRAGMENT_SOURCE, shaderc::ShaderKind::Fragment, "fragment.frag", "main", None)
			.map_err(|e| CreationError::ShaderCError (e))?;
		
		// Make into shader module
		let vertex_shader_module = unsafe {
			device
				.create_shader_module(vertex_compile_artifact.as_binary_u8())
				.map_err(|e| CreationError::ShaderModuleFailed (e))?
		};
		let fragment_shader_module = unsafe {
			device
				.create_shader_module(fragment_compile_artifact.as_binary_u8())
				.map_err(|e| CreationError::ShaderModuleFailed (e))?
		};

		// Specify entrypoints for each shader.
		let vs_entry: EntryPoint<Backend> = EntryPoint {
			entry: "main",
			module: &vertex_shader_module,
			specialization: Specialization {
				constants: Cow::Borrowed (&[]),
				data: Cow::Borrowed (&[]),
			}
		};

		let fs_entry: EntryPoint<Backend> = EntryPoint {
			entry: "main",
			module: &fragment_shader_module,
			specialization: Specialization {
				constants: Cow::Borrowed (&[]),
				data: Cow::Borrowed (&[]),
			}
		};

		// Specify input format
		let input_assembler = InputAssemblerDesc::new(hal::Primitive::TriangleList);

		// Vertex Shader I/O
		let vertex_buffers: Vec<VertexBufferDesc> = vec![VertexBufferDesc {
			binding: 0,
			stride: (size_of::<f32>() * 2) as u32,
			rate: VertexInputRate::Vertex,
		}];

		let attributes: Vec<AttributeDesc> = vec![AttributeDesc {
			location: 0,
			binding: 0,
			element: Element {
				format: Format::Rgb32Sfloat,
				offset: 0,
			},
		}];

		// Make shader set
		let shaders = GraphicsShaderSet {
			vertex: vs_entry,
			hull: None,
			domain: None,
			geometry: None,
			fragment: Some(fs_entry),
		};


		// Rasterisation options
		let rasterizer = Rasterizer {
			depth_clamping: false,
			polygon_mode: PolygonMode::Fill,
			cull_face: Face::NONE,
			front_face: FrontFace::Clockwise,
			depth_bias: None,
			conservative: false,
		};

		// Depth testing options
		let depth_stencil = DepthStencilDesc {
			depth: DepthTest::Off,
			depth_bounds: false,
			stencil: StencilTest::Off,
		};

		// Colour blending options
		// Only takes the source value
		let blender = {
			let blend_state = BlendState::On {
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
				targets: vec![ColorBlendDesc(ColorMask::ALL, blend_state)],
			}
		};

		// Viewport, scissor, options
		let baked_states = BakedStates {
			viewport: Some(Viewport {
				rect: extent.to_extent().rect(),
				depth: (0.0..1.0),
			}),
			scissor: Some(extent.to_extent().rect()),
			blend_color: None,
			depth_bounds: None,
		};

		// Non-Buffer data sources (none right now)
		let bindings = Vec::<DescriptorSetLayoutBinding>::new();
		let immutable_samplers = Vec::<<Backend as hal::Backend>::Sampler>::new();
		let descriptor_set_layouts: Vec<<Backend as hal::Backend>::DescriptorSetLayout> = vec![unsafe {
			device
				.create_descriptor_set_layout(bindings, immutable_samplers)
				.map_err(|e| CreationError::DescriptorSetLayoutFailed (e))?
		}];
		let push_constants = Vec::<(ShaderStageFlags, core::ops::Range<u32>)>::new();
		let layout = unsafe {
			device
				.create_pipeline_layout(&descriptor_set_layouts, push_constants)
				.map_err(|e| CreationError::PipelineLayoutFailed (e))?
		};

		// Create the actual pipeline
		let gfx_pipeline = {
			let desc = GraphicsPipelineDesc {
				shaders,
				rasterizer,
				vertex_buffers,
				attributes,
				input_assembler,
				blender,
				depth_stencil,
				multisampling: None,
				baked_states,
				layout: &layout,
				subpass: Subpass {
					index: 0,
					main_pass: render_pass,
				},
				flags: PipelineCreationFlags::empty(),
				parent: BasePipeline::None,
			};

			unsafe {
				device.create_graphics_pipeline(&desc, None)
					.map_err(|e| CreationError::PipelineFailed (e))?
			}
		};

		// TODO: Destroy shader modules
		unsafe {
			device.destroy_shader_module(vertex_shader_module);
			device.destroy_shader_module(fragment_shader_module);
		}

		Ok((descriptor_set_layouts, layout, gfx_pipeline))
    }

	/// Internal function. Gets the index of the next image from the swapchain to use & resets the frame_presented fence.
	fn get_image(swapchain: &mut <Backend as hal::Backend>::Swapchain, device: &mut <Backend as hal::Backend>::Device, cell: &FrameCell) -> Result<usize, FrameError> {
		// Get the image of the swapchain to present to.
		let (i, _) = unsafe {
			swapchain
				.acquire_image(core::u64::MAX, Some(&cell.image_available), None)
				.map_err(|e| FrameError::AcquisitionError { 0: e })?
		};
		let i = i as usize;

		// Make sure frame has been presented since whenever it was last drawn.
		unsafe {
			device
				.wait_for_fence(&cell.frame_presented, core::u64::MAX)
				.map_err(|e| FrameError::FenceWaitError { 0: e })?;
			device
				.reset_fence(&cell.frame_presented)
				.map_err(|e| FrameError::FenceResetError { 0: e })?;
		}

		Ok(i)
	}

	/// Internal function. Prepares a submission for a frame.
	fn prep_submission(cell: &FrameCell) 
	  -> Submission<ArrayVec<[&CommandBuffer<Backend, Graphics>; 1]>, 
					ArrayVec<[(&<Backend as hal::Backend>::Semaphore, hal::pso::PipelineStage); 1]>, 
					ArrayVec<[&<Backend as hal::Backend>::Semaphore; 1]>> {
		let command_buffers: ArrayVec<[_; 1]> = [&cell.command_buffer].into();

		let wait_semaphores: ArrayVec<[_; 1]> = [(&cell.image_available, PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
		let signal_semaphores: ArrayVec<[_; 1]> = [&cell.render_finished].into();

		Submission {
			command_buffers,
			wait_semaphores,
			signal_semaphores,
		}
	}

	/// Draw a frame of color 
	pub fn draw_clear(&mut self, color: [f32; 4]) -> Result<(), FrameError> {
		// Advance the frame before early outs to prevent fuckery.
		self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

		let i = RenderingContext::get_image(&mut self.swapchain, &mut self.device, &self.cells[self.current_frame])?;

		let cell = &mut self.cells[self.current_frame];

		// Record commands.
		unsafe {
			cell.command_buffer.begin();

			let clear_values = [ClearValue::Color(ClearColor::Float(color))];
			cell.command_buffer.begin_render_pass_inline(
				&self.render_pass,
				&self.framebuffers[i],
				self.render_area,
				clear_values.iter(),
			);
			cell.command_buffer.finish();
		};


		// Prepare submission
		let submission = RenderingContext::prep_submission(&cell);
		
		// Submit it for rendering and presentation.
		let command_queue = &mut self.queue_group.queues[0];

		let present_wait_semaphores: ArrayVec<[_; 1]> = [&cell.render_finished].into();

		unsafe {
			command_queue.submit(submission, Some(&cell.frame_presented));
			self.swapchain
				.present(command_queue, i as u32, present_wait_semaphores)
				.map_err(|e| FrameError::PresentError { 0: e })?
		};

		Ok(())
	}

	/// Draw a single triangle as a frame.
	pub fn draw_triangle(&mut self, triangle: Tri2) -> Result<(), FrameError> {
		// Advance the frame before early outs to prevent fuckery.
		self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

		// Get the image
		let i = RenderingContext::get_image(&mut self.swapchain, &mut self.device, &self.cells[self.current_frame])?;

		let cell = &mut self.cells[self.current_frame];

		// Write the vertex data to the buffer.
		unsafe {
			let mut data_target = self.device
				.acquire_mapping_writer(&self.memory, 0..self.requirements.size)
				.map_err(|e| FrameError::BufferError (e))?;
			
			let points: [f32; 6] = triangle.into();
			data_target[..6].copy_from_slice(&points);
			
			self
				.device
				.release_mapping_writer(data_target)
				.map_err(|e| FrameError::BufferError (hal::mapping::Error::OutOfMemory(e)))?;
		}

		// Record commands.
		unsafe {
			
			const TRIANGLE_CLEAR: [ClearValue; 1] = [ClearValue::Color(ClearColor::Float([0.1, 0.2, 0.3, 1.0]))];
			
			cell.command_buffer.begin();

			{
				let mut encoder = cell.command_buffer.begin_render_pass_inline(
					&self.render_pass,
					&self.framebuffers[i],
					self.render_area,
					TRIANGLE_CLEAR.iter(),
				);
				encoder.bind_graphics_pipeline(&self.pipeline);
				
				let buffer_ref: &<Backend as hal::Backend>::Buffer = &self.buffer;
				let buffers: ArrayVec<[_; 1]> = [(buffer_ref, 0)].into();
				
				encoder.bind_vertex_buffers(0, buffers);
				encoder.draw(0..3, 0..1);
			}
			cell.command_buffer.finish();
		}


		// Prepare submission
		let submission = RenderingContext::prep_submission(&cell);
		
		// Submit it for rendering and presentation.
		let command_queue = &mut self.queue_group.queues[0];

		let present_wait_semaphores: ArrayVec<[_; 1]> = [&cell.render_finished].into();

		unsafe {
			command_queue.submit(submission, Some(&cell.frame_presented));
			self.swapchain
				.present(command_queue, i as u32, present_wait_semaphores)
				.map_err(|e| FrameError::PresentError { 0: e })?;
			println!("presented");
		};
		Ok(())
	}
}

/// Properly destroys all the vulkan objects we have.
impl<'a> std::ops::Drop for RenderingContext<'a> {
	fn drop(&mut self) {
		use core::ptr::read;
		let _ = self.device.wait_idle();
	
		unsafe {
			// cells (semaphores, fences & command buffers)
			for cell in self.cells.drain(..) {
				cell.destroy(&self.device, &mut self.command_pool);
			}

			// images
			for image in self.backbuffer.drain(..) {
				self.device.destroy_image(image);
			}

			// image views
			for image_view in self.image_views.drain(..) {
				self.device.destroy_image_view(image_view);
			}

			// framebuffers
			for framebuffer in self.framebuffers.drain(..) {
				self.device.destroy_framebuffer(framebuffer);
			}

			for descriptor_set in self.descriptor_set_layouts.drain(..) {
				self.device.destroy_descriptor_set_layout(descriptor_set);
			}

			// buffer
			self.device.destroy_buffer(ManuallyDrop::into_inner(read(&self.buffer)));
			self.device.free_memory(ManuallyDrop::into_inner(read(&self.memory)));

			// graphics pipeline
			self.device
			  .destroy_pipeline_layout(ManuallyDrop::into_inner(read(&self.pipeline_layout)));

			self.device
			  .destroy_graphics_pipeline(ManuallyDrop::into_inner(read(&self.pipeline)));


			// command pool
			self.device
			  .destroy_command_pool(ManuallyDrop::into_inner(read(&self.command_pool)).into_raw());

			// render pass
			self.device
			  .destroy_render_pass(ManuallyDrop::into_inner(read(&self.render_pass)));

			// swapchain
			self.device
			  .destroy_swapchain(ManuallyDrop::into_inner(read(&self.swapchain)));

			ManuallyDrop::drop(&mut self.queue_group);
			ManuallyDrop::drop(&mut self.device);
			ManuallyDrop::drop(&mut self.instance);
		}
	}
}