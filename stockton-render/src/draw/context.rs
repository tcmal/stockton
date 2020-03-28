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
use crate::error as error;
use crate::error::{CreationError, FrameError};

use std::mem::{ManuallyDrop, size_of};
use std::convert::TryInto;

use winit::{EventsLoop, WindowBuilder};

use arrayvec::ArrayVec;

use hal::*;
use hal::device::Device;
use hal::format::{AsFormat, Rgba8Srgb as ColorFormat, Format, ChannelType};
use hal::pool::CommandPool;
use hal::queue::{QueueGroup, Submission};
use hal::window::SwapchainConfig;

use hal::Instance as InstanceTrait;

#[cfg(feature = "gl")]
use back::glutin as glutin;

use stockton_types::Vector2;
use super::buffer::StagedBuffer;

type ModifiableBuffer<'a> = StagedBuffer<'a>;

const ENTRY_NAME: &str = "main";
const COLOR_RANGE: image::SubresourceRange = image::SubresourceRange {
	aspects: format::Aspects::COLOR,
    levels: 0..1,
    layers: 0..1,
};

const VERTEX_SOURCE: &str = include_str!("./data/stockton.vert");
const FRAGMENT_SOURCE: &str = include_str!("./data/stockton.frag");

const VERTEX_BUFFER_BATCH_SIZE: u64 = 10;
const VERTEX_BUFFER_INITIAL_BATCHES: u64 = 1;

/// Represents a triangle in 2D (screen) space.
#[derive(Debug, Clone, Copy)]
pub struct Tri2 (pub [Vector2; 3]);

/// Easy conversion to proper format.
impl From<Tri2> for [f32; 15] {
	fn from(tri: Tri2) -> [f32; 15] {
	    [tri.0[0].x, tri.0[0].y, 1.0, 0.0, 0.0,
	     tri.0[1].x, tri.0[1].y, 0.0, 1.0, 0.0,
	     tri.0[2].x, tri.0[2].y, 0.0, 0.0, 1.0]
	}
}

const TRI2_SIZE_F32: usize = 15;
const TRI2_SIZE_BYTES: usize = size_of::<f32>() * TRI2_SIZE_F32;

#[cfg(not(feature = "gl"))]
type Instance = back::Instance;

#[cfg(feature = "gl")]
type Instance = ();

/// Contains all the hal related stuff.
/// In the end, this takes some 3D points and puts it on the screen.
// TODO: Settings for clear colour, buffer sizes, etc
pub struct RenderingContext<'a> {
	pub events_loop: winit::EventsLoop,
	surface: <back::Backend as hal::Backend>::Surface,

	pub (crate) instance: ManuallyDrop<Instance>,
	pub (crate) device: ManuallyDrop<<back::Backend as hal::Backend>::Device>,

	swapchain: ManuallyDrop<<back::Backend as hal::Backend>::Swapchain>,
	
	viewport: pso::Viewport,

	imageviews: Vec<<back::Backend as hal::Backend>::ImageView>,
	framebuffers: Vec<<back::Backend as hal::Backend>::Framebuffer>,

	renderpass: ManuallyDrop<<back::Backend as hal::Backend>::RenderPass>,

	current_frame: usize,
	// TODO: Collect these together
	get_image: Vec<<back::Backend as hal::Backend>::Semaphore>,
	render_complete: Vec<<back::Backend as hal::Backend>::Semaphore>,
	present_complete: Vec<<back::Backend as hal::Backend>::Fence>,

	frames_in_flight: usize,
	cmd_pools: Vec<ManuallyDrop<<back::Backend as hal::Backend>::CommandPool>>,
	cmd_buffers: Vec<<back::Backend as hal::Backend>::CommandBuffer>,
	queue_group: QueueGroup<back::Backend>,

	vert_buffer: ModifiableBuffer<'a>,
	index_buffer: ModifiableBuffer<'a>,

	descriptor_set_layouts: <back::Backend as hal::Backend>::DescriptorSetLayout,
	pipeline_layout: ManuallyDrop<<back::Backend as hal::Backend>::PipelineLayout>,
	pipeline: ManuallyDrop<<back::Backend as hal::Backend>::GraphicsPipeline>,
	pub (crate) adapter: adapter::Adapter<back::Backend>
}

impl<'a> RenderingContext<'a> {
	/// Create a new RenderingContext for the given window.
	pub fn new() -> Result<Self, CreationError> {
	    let events_loop = EventsLoop::new();
	    let wb = WindowBuilder::new();

	    // Create surface
	    #[cfg(not(feature = "gl"))]
	    let (window, instance, mut surface, mut adapters) = {
	    	use hal::Instance;
	    	let window = wb.build(&events_loop).map_err(|_| CreationError::WindowError)?;
	        let instance = back::Instance::create("stockton", 1);
	        let surface = instance.create_surface(&window);
	        let adapters = instance.enumerate_adapters();

	        (window, instance, surface, adapters)
	    };

	    #[cfg(feature = "gl")]
	    let (window, instance, mut surface, mut adapters) = {
	    	use back::glutin::ContextBuilder;

			let glutin_window = ContextBuilder::new().with_vsync(true).build_windowed(wb, &events_loop).unwrap();
			let (glutin_context, glutin_window) = unsafe {
				glutin_window.make_current().map_err(|_| CreationError::WindowError)?
					.split()
			};

			let surface = back::Surface::from_context(glutin_context);
	    	let adapters = surface.enumerate_adapters();

	    	((), (), surface, adapters)
	    };

	    // TODO: Properly figure out which adapter to use
	    let mut adapter = adapters.remove(0);

	    // Device & Queue group
	    let (mut device, queue_group) = {
	    	// TODO
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

	        (gpu.queue_groups.pop().unwrap(), gpu.device)
	    };

	    // Swapchain stuff
	    let (format, viewport, extent, swapchain, backbuffer) = {
            use hal::window::{PresentMode, CompositeAlphaMode};

		    let (caps, formats, present_modes) = surface.compatibility(&mut adapter.physical_device);

		    let format = formats.map_or(Format::Rgba8Srgb, |formats| {
		    	formats.iter()
		    		.find(|format| format.base_format().1 == ChannelType::Srgb)
		    		.map(|format| *format)
		    		.unwrap_or(formats[0])
		    });

            let present_mode = {
                [PresentMode::Mailbox, PresentMode::Fifo, PresentMode::Relaxed, PresentMode::Immediate]
                    .iter()
                    .cloned()
                    .find(|pm| present_modes.contains(pm))
                    .ok_or(CreationError::BadSurface)?
            };
            let composite_alpha = {
                [CompositeAlphaMode::OPAQUE, CompositeAlphaMode::INHERIT, CompositeAlphaMode::PREMULTIPLIED, CompositeAlphaMode::POSTMULTIPLIED]
                    .iter()
                    .cloned()
                    .find(|ca| caps.composite_alpha.contains(*ca))
                    .ok_or(CreationError::BadSurface)?
            };

            let extent = caps.extents.end();
            let image_count = if present_mode == PresentMode::Mailbox {
                ((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(3))
            } else {
                ((*caps.image_count.end()) - 1).min((*caps.image_count.start()).max(2))
            };

            let image_layers = 1;
            let image_usage = if caps.usage.contains(image::Usage::COLOR_ATTACHMENT) {
                image::Usage::COLOR_ATTACHMENT
            } else {
                Err(CreationError::BadSurface)?
			};

		    // Swap config
		    let swap_config = SwapchainConfig {
                present_mode,
                composite_alpha,
                format,
                extent: *extent,
                image_count,
                image_layers,
                image_usage,
			};

		    // Viewport
		    let extent = extent.to_extent();
		    let viewport = pso::Viewport {
		    	rect: extent.rect(),
		    	depth: 0.0..1.0
		    };
		    
		    // Swapchain
		    let (swapchain, backbuffer) = unsafe {
		    	device.create_swapchain(&mut surface, swap_config, None)
		    		.map_err(|e| CreationError::SwapchainError (e))?
		    };

		    (format, viewport, extent, swapchain, backbuffer)
	    };

		// Renderpass
		let renderpass = {
			use hal::pass::*;
			use hal::pso::PipelineStage;
			use hal::image::{Access, Layout};

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
				passes: None..0,
				stages: PipelineStage::COLOR_ATTACHMENT_OUTPUT..PipelineStage::COLOR_ATTACHMENT_OUTPUT,
				accesses: Access::empty()
	                ..(Access::COLOR_ATTACHMENT_READ | Access::COLOR_ATTACHMENT_WRITE)
			};

			unsafe { device.create_render_pass(&[attachment], &[subpass], &[dependency]) }
	            .map_err(|_| CreationError::OutOfMemoryError)?
		};

		// Subpass
		let subpass = pass::Subpass {
			index: 0,
			main_pass: &renderpass
		};

	    // Vertex and index buffers
	    let (vert_buffer, index_buffer) = {
	    	use hal::buffer::Usage;
	    	(
				ModifiableBuffer::new(&mut device, &adapter, Usage::VERTEX | Usage::TRANSFER_DST),
				ModifiableBuffer::new(&mut device, &adapter, Usage::TRANSFER_SRC)
	    	)
	    };

	    // Command Pools, Buffers, imageviews, framebuffers & Sync objects
    	let frames_in_flight = backbuffer.len();
	    let (cmd_pools, cmd_buffers, get_image, render_complete, present_complete, imageviews, framebuffers) = {
		    let mut cmd_pools = Vec::with_capacity(frames_in_flight);
		    let mut cmd_buffers = Vec::with_capacity(frames_in_flight);
		    let mut get_image = Vec::with_capacity(frames_in_flight);
		    let mut render_complete = Vec::with_capacity(frames_in_flight);
		    let mut present_complete = Vec::with_capacity(frames_in_flight);
		    let mut imageviews = Vec::with_capacity(frames_in_flight);
		    let mut framebuffers = Vec::with_capacity(frames_in_flight);

		    for i in 0..frames_in_flight {
			    cmd_pools.push(ManuallyDrop::new(unsafe {
			        device.create_command_pool_typed(&queue_group, pool::CommandPoolCreateFlags::empty())
			    }.map_err(|_| CreationError::OutOfMemoryError)?));

			    cmd_buffers.push((*cmd_pools[i]).allocate_one(hal::command::Level::Primary));
			    get_image.push(device.create_semaphore().map_err(|_| CreationError::SyncObjectError)?);
			    render_complete.push(device.create_semaphore().map_err(|_| CreationError::SyncObjectError)?);
			    present_complete.push(device.create_fence(true).map_err(|_| CreationError::SyncObjectError)?);
		    	
		    	unsafe {
			    	imageviews.push(device.create_image_view(
		                &backbuffer[i],
		                image::ViewKind::D2,
		                format,
		                format::Swizzle::NO,
		                COLOR_RANGE.clone(),
		            ).map_err(|e| CreationError::ImageViewError (e))?);
		            framebuffers.push(device.create_framebuffer(
		            	&renderpass,
		            	Some(&imageviews[i]),
		            	extent
		            ).map_err(|_| CreationError::OutOfMemoryError)?);
		    	}
		    }

		    (cmd_pools, cmd_buffers, get_image, render_complete, present_complete, imageviews, framebuffers)
	    };

	    // Graphics pipeline
	    let (descriptor_set_layouts, pipeline_layout, pipeline) = Self::create_pipeline(&mut device, extent, &subpass)?;

    	Ok(RenderingContext {
    		instance: ManuallyDrop::new(instance),
    		events_loop,
    		surface,

    		device: ManuallyDrop::new(device),
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
    		cmd_pools,
    		cmd_buffers,

    		descriptor_set_layouts: descriptor_set_layouts,
    		pipeline_layout: ManuallyDrop::new(pipeline_layout),
    		pipeline: ManuallyDrop::new(pipeline),

    		vert_buffer,
    		index_buffer,

    		adapter
    	})
	}

	#[allow(clippy::type_complexity)]
	pub fn create_pipeline(device: &mut <back::Backend as hal::Backend>::Device, extent: image::Extent, subpass: &pass::Subpass<back::Backend>) -> Result<
    (
      <back::Backend as hal::Backend>::DescriptorSetLayout,
      <back::Backend as hal::Backend>::PipelineLayout,
      <back::Backend as hal::Backend>::GraphicsPipeline,
    ), error::CreationError> {
    	use hal::pso::*;

        // Shader modules
        let (vs_module, fs_module) = {
			let mut compiler = shaderc::Compiler::new().ok_or(error::CreationError::NoShaderC)?;

			let vertex_compile_artifact = compiler
				.compile_into_spirv(VERTEX_SOURCE, shaderc::ShaderKind::Vertex, "vertex.vert", "main", None)
				.map_err(|e| error::CreationError::ShaderCError (e))?;
			
			let fragment_compile_artifact = compiler
				.compile_into_spirv(FRAGMENT_SOURCE, shaderc::ShaderKind::Fragment, "fragment.frag", "main", None)
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
			stride: (size_of::<f32>() * 5) as u32,
			rate: VertexInputRate::Vertex,
		}];

		let attributes: Vec<AttributeDesc> = vec![AttributeDesc {
			location: 0,
			binding: 0,
			element: Element {
				format: Format::Rg32Sfloat,
				offset: 0,
			},
		}, AttributeDesc {
			location: 1,
			binding: 0,
			element: Element {
				format: Format::Rgb32Sfloat,
				offset: (size_of::<f32>() * 2) as ElemOffset,
			}
		}];

    	// Rasterizer
    	let rasterizer = Rasterizer {
    		polygon_mode: PolygonMode::Fill,
    		cull_face: Face::NONE,
    		front_face: FrontFace::Clockwise,
    		depth_clamping: false,
    		depth_bias: None,
    		conservative: true
    	};

    	// Depth stencil
		let depth_stencil = DepthStencilDesc {
			depth: None,
			depth_bounds: false,
			stencil: None,
		};

		// Descriptor set layout
    	let set_layout = unsafe {
	        device.create_descriptor_set_layout(
	            &[],
	            &[],
	        )
	    }.map_err(|_| error::CreationError::OutOfMemoryError)?;

    	// Pipeline layout
    	let layout = unsafe {
    		device.create_pipeline_layout(std::iter::once(&set_layout), &[])
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
    		flags: pso::PipelineCreationFlags::empty(),
    		parent: pso::BasePipeline::None,
    		input_assembler,
    		attributes
    	};

    	// Pipeline
    	let pipeline = unsafe {
    		device.create_graphics_pipeline(&pipeline_desc, None)
    	}.map_err(|e| error::CreationError::PipelineError (e))?;

    	Ok((set_layout, layout, pipeline))
	}

    /// Draw a frame that's just cleared to the color specified.
    pub fn draw_clear(&mut self, color: [f32; 4]) -> Result<(), FrameError> {
        let get_image = &self.get_image[self.current_frame];
        let render_complete = &self.render_complete[self.current_frame];
        
        // Advance the frame _before_ we start using the `?` operator
        self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

        // Get the image
        let (image_index, _) = unsafe {
            self
                .swapchain
                .acquire_image(core::u64::MAX, Some(get_image), None)
                .map_err(|e| FrameError::AcquireError (e))?
        };
        let image_index = image_index as usize;

        // Make sure whatever was last using this has finished
        let present_complete = &self.present_complete[image_index];
        unsafe {
            self.device
                .wait_for_fence(present_complete, core::u64::MAX)
                .map_err(|_| FrameError::SyncObjectError)?;
            self.device
                .reset_fence(present_complete)
                .map_err(|_| FrameError::SyncObjectError)?;
        };

        // Record commands
        unsafe {
            let buffer = &mut self.cmd_buffers[image_index];
            let clear_values = [command::ClearValue::Color(command::ClearColor::Sfloat(color))];

            buffer.begin(false);
            buffer.begin_render_pass_inline(
                &self.renderpass,
                &self.framebuffers[image_index],
                self.viewport.rect,
                clear_values.iter(),
            );
            buffer.finish();
        };

        // Make submission object
        let command_buffers = &self.cmd_buffers[image_index..=image_index];
        let wait_semaphores: ArrayVec<[_; 1]> = [(get_image, pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
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
                .map_err(|_| FrameError::PresentError)?
        };

        Ok(())
	}

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
            let buffer = &mut self.cmd_buffers[image_index];
            let clear_values = [command::ClearValue::Color(command::ClearColor::Sfloat([0.0, 0.0, 0.0, 1.0]))];

            buffer.begin(false);
            {
				let mut encoder = buffer.begin_render_pass_inline(
					&self.renderpass,
					&self.framebuffers[image_index],
					self.viewport.rect,
					clear_values.iter(),
				);
				encoder.bind_graphics_pipeline(&self.pipeline);
				
				// Here we must force the Deref impl of ManuallyDrop to play nice.
				let buffer_ref: &<back::Backend as hal::Backend>::Buffer = &self.map_verts.buffer;
				let buffers: ArrayVec<[_; 1]> = [(buffer_ref, 0)].into();

				encoder.bind_vertex_buffers(0, buffers);

				trace!("Requesting draw of {:?} instances ({:?} verts)", self.map_verts.active_instances, self.map_verts.active_verts);
				encoder.draw(self.map_verts.active_verts.clone(), self.map_verts.active_instances.clone());
			}
            buffer.finish();
        };

        // Make submission object
        let command_buffers = &self.cmd_buffers[image_index..=image_index];
        let wait_semaphores: ArrayVec<[_; 1]> = [(get_image, pso::PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
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

	pub fn add_map_vert(&mut self, tri: Tri2) -> Result<(), ()> {
		// get around the borrow checker
		unsafe {
			let ctx: *mut Self = &mut *self;
			self.map_verts.add(tri, ctx.as_mut().unwrap())
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

            // self.map_verts.deactivate(self);

            use core::ptr::read;
            for cmd_pool in self.cmd_pools.drain(..) {
	            self.device.destroy_command_pool(
	                ManuallyDrop::into_inner(cmd_pool),
	            );
            }
            self.device
                .destroy_render_pass(ManuallyDrop::into_inner(read(&self.renderpass)));
            self.device
                .destroy_swapchain(ManuallyDrop::into_inner(read(&self.swapchain)));
            
            ManuallyDrop::drop(&mut self.device);
		}
	}
}