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
extern crate gfx_hal as hal;
extern crate shaderc;
extern crate winit;

extern crate image;
extern crate log;
extern crate nalgebra_glm as na;

extern crate stockton_levels;
extern crate stockton_types;

extern crate arrayvec;

mod culling;
pub mod draw;
mod error;
mod types;

use stockton_levels::prelude::*;
use stockton_types::World;

use culling::get_visible_faces;
use draw::RenderingContext;
use error::{CreationError, FrameError};

/// Renders a world to a window when you tell it to.
pub struct Renderer<'a, T: MinBSPFeatures<VulkanSystem>> {
    world: World<T>,
    pub context: RenderingContext<'a>,
}

impl<'a, T: MinBSPFeatures<VulkanSystem>> Renderer<'a, T> {
    /// Create a new Renderer.
    /// This initialises all the vulkan context, etc needed.
    pub fn new(world: World<T>, window: &winit::window::Window) -> Result<Self, CreationError> {
        let context = RenderingContext::new(window, &world.map)?;

        Ok(Renderer { world, context })
    }

    /// Render a single frame of the world
    pub fn render_frame(&mut self) -> Result<(), FrameError> {
        // Get visible faces
        let faces = get_visible_faces(self.context.camera_pos(), &self.world.map);

        // Then draw them
        if self.context.draw_vertices(&self.world.map, &faces).is_err() {
            unsafe { self.context.handle_surface_change().unwrap() };

            // If it fails twice, then error
            self.context
                .draw_vertices(&self.world.map, &faces)
                .map_err(|_| FrameError::PresentError)?;
        }

        Ok(())
    }
}
