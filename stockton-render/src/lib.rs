/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;
extern crate nalgebra_glm as na;

#[macro_use]
extern crate legion;

mod culling;
pub mod draw;
mod error;
mod types;
pub mod window;

use culling::get_visible_faces;
use draw::RenderingContext;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::RwLock;
pub use window::WindowEvent;

use stockton_levels::prelude::*;
use winit::event_loop::ControlFlow;
use winit::window::Window;

use std::sync::mpsc::channel;

/// Renders a world to a window when you tell it to.
/// Also takes ownership of the window and channels window events to be processed outside winit's event loop.
pub struct Renderer<'a> {
    /// All the vulkan stuff
    context: RenderingContext<'a>,

    /// For getting events from the winit event loop
    pub window_events: Receiver<WindowEvent>,

    /// For updating the control flow of the winit event loop
    pub update_control_flow: Arc<RwLock<ControlFlow>>,
}

impl<'a> Renderer<'a> {
    /// Create a new Renderer.
    pub fn new<T: MinBSPFeatures<VulkanSystem>>(
        window: &Window,
        file: &T,
    ) -> (Self, Sender<WindowEvent>) {
        let (tx, rx) = channel();
        let update_control_flow = Arc::new(RwLock::new(ControlFlow::Poll));

        (
            Renderer {
                context: RenderingContext::new(window, file).unwrap(),
                window_events: rx,
                update_control_flow,
            },
            tx,
        )
    }

    /// Render a single frame of the given map.
    fn render<T: MinBSPFeatures<VulkanSystem>>(&mut self, map: &T) {
        // Get visible faces
        let faces = get_visible_faces(self.context.camera_pos(), map);

        // Then draw them
        if self.context.draw_vertices(map, &faces).is_err() {
            unsafe { self.context.handle_surface_change().unwrap() };

            // If it fails twice, then error
            self.context.draw_vertices(map, &faces).unwrap();
        }
    }
}

/// A system that just renders the world.
#[system]
pub fn do_render<T: 'static + MinBSPFeatures<VulkanSystem>>(
    #[resource] renderer: &mut Renderer<'static>,
    #[resource] map: &T,
) {
    renderer.render(map);
}
