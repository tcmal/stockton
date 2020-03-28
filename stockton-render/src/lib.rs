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

extern crate core;

#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;

extern crate log;
extern crate gfx_hal as hal;
extern crate stockton_types;
extern crate shaderc;
extern crate winit;

extern crate arrayvec;

pub mod draw;
mod error;
mod types;

use std::sync::{Arc, RwLock};

use stockton_types::World;

use error::{CreationError, FrameError};
use draw::RenderingContext;

/// Renders a world to a window when you tell it to.
pub struct Renderer<'a> {
	_world: Arc<RwLock<World<'a>>>,
	pub context: RenderingContext<'a>
}


impl<'a> Renderer<'a> {
	/// Create a new Renderer.
	/// This initialises all the vulkan context, etc needed.
	pub fn new(world: Arc<RwLock<World<'a>>>, window: &winit::window::Window) -> Result<Self, CreationError> {
		let context = RenderingContext::new(window)?;

		Ok(Renderer {
			_world: world, context
		})
	}

	/// Render a single frame of the world
	pub fn render_frame(&mut self) -> Result<(), FrameError>{
		self.context.draw_vertices().unwrap();
		Ok(())
	}
}
