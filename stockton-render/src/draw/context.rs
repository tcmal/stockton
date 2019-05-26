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
use core::mem::ManuallyDrop;

use crate::error::{CreationError, FrameError};
use super::frame::FrameCell;

use arrayvec::ArrayVec;

use winit::Window;

// Trait imports
use hal::{Surface as SurfaceTrait, Instance as InstanceTrait, QueueFamily as QFTrait, PhysicalDevice as PDTrait, Device as DeviceTrait, Swapchain as SwapchainTrait};

use hal::{Graphics, Gpu, Features, SwapchainConfig, Submission};
use hal::pass::{SubpassDesc, AttachmentOps, Attachment, AttachmentStoreOp, AttachmentLoadOp};
use hal::image::{Usage, Layout, SubresourceRange, ViewKind, Extent};
use hal::format::{ChannelType, Format, Swizzle, Aspects};
use hal::pool::{CommandPoolCreateFlags, CommandPool};
use hal::command::{ClearValue, ClearColor};
use hal::pso::{Rect, PipelineStage};
use hal::queue::family::QueueGroup;
use hal::window::{PresentMode, Extent2D};
use hal::adapter::Adapter;

use back::{Instance};
use back::{Backend};

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
	current_frame: usize
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
		let (device, queue_group) = {
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
	
			current_frame: 0
		})
	}

	/// Draw a frame of color 
	pub fn draw_clear(&mut self, color: [f32; 4]) -> Result<(), FrameError> {
		// Advance the frame before early outs to prevent fuckery.
		self.current_frame = (self.current_frame + 1) % self.frames_in_flight;

		let cell = &mut self.cells[self.current_frame];

		// Get the image of the swapchain to present to.
		let (i, _) = unsafe {
			self.swapchain
				.acquire_image(core::u64::MAX, Some(&cell.image_available), None)
				.map_err(|e| FrameError::AcquisitionError { 0: e })?
		};
		let i = i as usize;

		// Make sure frame has been presented since whenever it was last drawn.
		unsafe {
			self.device
				.wait_for_fence(&cell.frame_presented, core::u64::MAX)
				.map_err(|e| FrameError::FenceWaitError { 0: e })?;
			self.device
				.reset_fence(&cell.frame_presented)
				.map_err(|e| FrameError::FenceResetError { 0: e })?;
		}

		// Record commands.
		unsafe {
			let clear_values = [ClearValue::Color(ClearColor::Float(color))];
			cell.command_buffer.begin();
			cell.command_buffer.begin_render_pass_inline(
				&self.render_pass,
				&self.framebuffers[i],
				self.render_area,
				clear_values.iter(),
			);
			cell.command_buffer.finish();
		}


		// Prepare submission
		let command_buffers: ArrayVec<[_; 1]> = [&cell.command_buffer].into();

		let wait_semaphores: ArrayVec<[_; 1]> = [(&cell.image_available, PipelineStage::COLOR_ATTACHMENT_OUTPUT)].into();
		let signal_semaphores: ArrayVec<[_; 1]> = [&cell.render_finished].into();
		let present_wait_semaphores: ArrayVec<[_; 1]> = [&cell.render_finished].into();
		let submission = Submission {
			command_buffers,
			wait_semaphores,
			signal_semaphores,
		};
		
		// Submit it for rendering and presentation.
		let command_queue = &mut self.queue_group.queues[0];

		unsafe {
			command_queue.submit(submission, Some(&cell.frame_presented));
			self.swapchain
				.present(command_queue, i as u32, present_wait_semaphores)
				.map_err(|e| FrameError::PresentError { 0: e })?
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