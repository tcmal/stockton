#![allow(incomplete_features)]
#![feature(generic_associated_types)]
#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;
extern crate nalgebra_glm as na;

#[macro_use]
extern crate legion;

pub mod draw;
pub mod error;
pub mod systems;
mod types;
pub mod window;

use draw::{
    RenderingContext,
    draw_passes::{DrawPass, IntoDrawPass}
};
use error::full_error_display;
use legion::world::SubWorld;
use legion::IntoQuery;
use std::sync::mpsc::{Receiver, Sender};
use std::sync::Arc;
use std::sync::RwLock;
pub use window::{UiState, WindowEvent};

use anyhow::Result;
use log::error;
use stockton_levels::prelude::*;
use stockton_types::components::{CameraSettings, Transform};
use stockton_types::{Vector3, Session};
use winit::event_loop::ControlFlow;
use winit::window::Window;

use std::sync::mpsc::channel;

/// Renders a world to a window when you tell it to.
/// Also takes ownership of the window and channels window events to be processed outside winit's event loop.
pub struct Renderer<DP> {
    /// All the vulkan stuff
    pub(crate) context: RenderingContext<DP>,

    /// For getting events from the winit event loop
    pub window_events: Receiver<WindowEvent>,

    /// For updating the control flow of the winit event loop
    pub update_control_flow: Arc<RwLock<ControlFlow>>,
}

impl<DP: DrawPass> Renderer<DP> {
    /// Create a new Renderer.
    pub fn new<IDP: IntoDrawPass<DP>>(window: &Window, ui: &mut UiState, idp: IDP) -> Result<(Self, Sender<WindowEvent>)> {
        let (tx, rx) = channel();
        let update_control_flow = Arc::new(RwLock::new(ControlFlow::Poll));

        Ok((
            Renderer {
                context: RenderingContext::new(window, idp)?,
                window_events: rx,
                update_control_flow,
            },
            tx,
        ))
    }

    /// Render a single frame of the given session.
    fn render(&mut self, session: &Session) -> Result<()> {
        // Try to draw
        if self.context.draw_next_frame(session).is_err() {
            // Probably the surface changed
            unsafe { self.context.handle_surface_change()? };

            // If it fails twice, then error
            self.context.draw_next_frame(session)?;
        }

        Ok(())
    }

    fn resize(&mut self) -> Result<()> {
        unsafe { self.context.handle_surface_change() }
    }
}