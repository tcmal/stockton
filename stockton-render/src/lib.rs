#[cfg(feature = "vulkan")]
extern crate gfx_backend_vulkan as back;
extern crate gfx_hal as hal;
extern crate nalgebra_glm as na;

#[macro_use]
extern crate derive_builder;

pub mod buffers;
pub mod builders;
pub mod context;
pub mod draw_passes;
pub mod error;
pub mod queue_negotiator;
mod target;
pub mod texture;
pub mod types;
pub mod utils;

use context::RenderingContext;
use draw_passes::{DrawPass, IntoDrawPass};

use anyhow::{Context, Result};

use stockton_types::Session;
use winit::window::Window;

/// Renders a world to a window when you tell it to.
/// Also takes ownership of the window and channels window events to be processed outside winit's event loop.
pub struct Renderer<DP> {
    /// All the vulkan stuff
    context: RenderingContext,

    /// The draw pass we're using
    draw_pass: DP,
}

impl<DP: DrawPass> Renderer<DP> {
    /// Create a new Renderer.
    pub fn new<IDP: IntoDrawPass<DP>>(
        window: &Window,
        session: &mut Session,
        idp: IDP,
    ) -> Result<Self> {
        let mut context = RenderingContext::new::<IDP, DP>(window)?;

        // Draw pass
        let draw_pass = idp
            .init(session, &mut context)
            .context("Error initialising draw pass")?;

        Ok(Renderer { context, draw_pass })
    }

    /// Render a single frame of the given session.
    pub fn render(&mut self, session: &Session) -> Result<()> {
        // Try to draw
        if self
            .context
            .draw_next_frame(session, &mut self.draw_pass)
            .is_err()
        {
            // Probably the surface changed
            self.handle_surface_change(session)?;

            // If it fails twice, then error
            self.context.draw_next_frame(session, &mut self.draw_pass)?;
        }

        Ok(())
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        let e = self.context.target_chain().properties().extent;
        e.width as f32 / e.height as f32
    }

    pub fn handle_surface_change(&mut self, session: &Session) -> Result<()> {
        unsafe {
            self.context.handle_surface_change()?;
            self.draw_pass
                .handle_surface_change(session, &mut self.context)?;
        }

        Ok(())
    }

    /// Get a reference to the renderer's context.
    pub fn context(&self) -> &RenderingContext {
        &self.context
    }
}
