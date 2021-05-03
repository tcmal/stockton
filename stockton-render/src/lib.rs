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
#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;
extern crate nalgebra_glm as na;

#[macro_use]
extern crate legion;

mod culling;
pub mod draw;
mod error;
pub mod systems;
mod types;
pub mod window;

use culling::get_visible_faces;
use draw::RenderingContext;
use legion::world::SubWorld;
use legion::IntoQuery;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::RwLock;
pub use window::{UiState, WindowEvent};

use stockton_levels::prelude::*;
use stockton_types::components::{CameraSettings, Transform};
use stockton_types::Vector3;
use winit::event_loop::ControlFlow;
use winit::window::Window;

use std::sync::mpsc::channel;

/// Renders a world to a window when you tell it to.
/// Also takes ownership of the window and channels window events to be processed outside winit's event loop.
pub struct Renderer<'a, M: 'static + MinBspFeatures<VulkanSystem>> {
    /// All the vulkan stuff
    pub(crate) context: RenderingContext<'a, M>,

    /// For getting events from the winit event loop
    pub window_events: Receiver<WindowEvent>,

    /// For updating the control flow of the winit event loop
    pub update_control_flow: Arc<RwLock<ControlFlow>>,
}

impl<'a, M: 'static + MinBspFeatures<VulkanSystem>> Renderer<'a, M> {
    /// Create a new Renderer.
    pub fn new(window: &Window, file: M) -> (Self, Sender<WindowEvent>) {
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
    fn render(&mut self, ui: &mut UiState, pos: Vector3) {
        // Get visible faces
        let faces = get_visible_faces(pos, &*self.context.map);

        // Then draw them
        if self.context.draw_vertices(ui, &faces).is_err() {
            unsafe { self.context.handle_surface_change().unwrap() };

            // If it fails twice, then error
            self.context.draw_vertices(ui, &faces).unwrap();
        }
    }

    fn resize(&mut self) {
        unsafe { self.context.handle_surface_change().unwrap() };
    }
}

/// A system that just renders the world.
#[system]
#[read_component(Transform)]
#[read_component(CameraSettings)]
pub fn do_render<T: 'static + MinBspFeatures<VulkanSystem>>(
    #[resource] renderer: &mut Renderer<'static, T>,
    #[resource] ui: &mut UiState,
    world: &SubWorld,
) {
    let mut query = <(&Transform, &CameraSettings)>::query();
    for (transform, _) in query.iter(world) {
        renderer.render(ui, transform.position);
    }
}
