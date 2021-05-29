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
pub mod error;
pub mod systems;
mod types;
pub mod window;

use culling::get_visible_faces;
use draw::RenderingContext;
use error::full_error_display;
use error::LockPoisoned;
use legion::world::SubWorld;
use legion::IntoQuery;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::RwLock;
pub use window::{UiState, WindowEvent};

use anyhow::{Context, Result};
use log::error;
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
    pub fn new(window: &Window, file: M) -> Result<(Self, Sender<WindowEvent>)> {
        let (tx, rx) = channel();
        let update_control_flow = Arc::new(RwLock::new(ControlFlow::Poll));

        Ok((
            Renderer {
                context: RenderingContext::new(window, file)?,
                window_events: rx,
                update_control_flow,
            },
            tx,
        ))
    }

    /// Render a single frame of the given map.
    fn render(&mut self, ui: &mut UiState, pos: Vector3) -> Result<()> {
        // Get visible faces
        let faces = get_visible_faces(
            pos,
            &*self
                .context
                .map
                .read()
                .map_err(|_| LockPoisoned::Map)
                .context("Error getting read lock on map")?,
        );

        // Then draw them
        if self.context.draw_vertices(ui, &faces).is_err() {
            unsafe { self.context.handle_surface_change()? };

            // If it fails twice, then error
            self.context.draw_vertices(ui, &faces)?;
        }

        Ok(())
    }

    fn resize(&mut self) -> Result<()> {
        unsafe { self.context.handle_surface_change() }
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
        if let Err(err) = renderer.render(ui, transform.position) {
            error!("{}", full_error_display(err));
        }
    }
}
