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

//! Renders a world to a window. 
//!
//! You'll need to pick a backend using features. You should only pick one.
//! On Linux & Windows, you should use vulkan.
//! On Mac, you should use `metal`.
//! If your targetting machines without Vulkan, OpenGL or dx11/dx12 is preferred.
//! `empty` is used for testing
#[cfg(feature = "dx11")]
extern crate gfx_backend_dx11 as back;

#[cfg(feature = "dx12")]
extern crate gfx_backend_dx12 as back;

#[cfg(feature = "gl")]
extern crate gfx_backend_gl as back;

#[cfg(feature = "metal")]
extern crate gfx_backend_metal as back;

#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

extern crate gfx_hal as hal;
extern crate stockton_types;
extern crate shaderc;
extern crate winit;

extern crate arrayvec;

mod error;
mod draw;

use error::{CreationError, FrameError};
use draw::{RenderingContext, Tri2};

use stockton_types::{World, Vector2};

use winit::Window;

use std::sync::{Arc, RwLock};

pub struct Renderer<'a> {
	world: Arc<RwLock<World<'a>>>,
	context: RenderingContext<'a>
}


impl<'a> Renderer<'a> {
	/// Create a new Renderer.
	/// This initialises all the vulkan context, etc needed.
	pub fn new(world: Arc<RwLock<World<'a>>>, window: &'a Window) -> Result<Self, CreationError> {
		let context = RenderingContext::new(window)?;

		Ok(Renderer {
			world, context
		})
	}

	/// Render a single frame of the world
	pub fn render_frame(&mut self) -> Result<(), FrameError>{
		// self.context.draw_clear([0.0, 0.5, 0.5, 1.0])
		self.context.draw_triangle(Tri2 ([
			Vector2::new(-0.5, -0.5),
			Vector2::new(0.5, -0.5),
			Vector2::new(0.0, 0.5)
		]))
	}
}
