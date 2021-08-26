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
pub mod mem;
pub mod queue_negotiator;
mod target;
pub mod texture;
pub mod types;
pub mod utils;

use std::mem::ManuallyDrop;

use context::RenderingContext;
use draw_passes::{DrawPass, IntoDrawPass, Singular};

use anyhow::{Context, Result};

use stockton_types::Session;
use winit::window::Window;

/// Renders a world to a window when you tell it to.
/// Also takes ownership of the window and channels window events to be processed outside winit's event loop.
pub struct Renderer<DP> {
    /// All the vulkan stuff
    context: ManuallyDrop<RenderingContext>,

    /// The draw pass we're using
    draw_pass: ManuallyDrop<DP>,
}

impl<DP: DrawPass<Singular>> Renderer<DP> {
    /// Create a new Renderer.
    pub fn new<IDP: IntoDrawPass<DP, Singular>>(
        window: &Window,
        session: &mut Session,
        idp: IDP,
    ) -> Result<Self> {
        let mut context = RenderingContext::new::<IDP, DP>(window)?;

        // Draw pass
        let draw_pass = idp
            .init(session, &mut context)
            .context("Error initialising draw pass")?;

        Ok(Renderer {
            context: ManuallyDrop::new(context),
            draw_pass: ManuallyDrop::new(draw_pass),
        })
    }

    /// Render a single frame of the given session.
    /// If this returns an error, the whole renderer is dead, hence it takes ownership to ensure it can't be called in that case.
    pub fn render(mut self, session: &Session) -> Result<Renderer<DP>> {
        // Safety: If this fails at any point, the ManuallyDrop won't be touched again, as Renderer will be dropped.
        // Hence, we can always take from the ManuallyDrop
        unsafe {
            match ManuallyDrop::take(&mut self.context)
                .draw_next_frame(session, &mut *self.draw_pass)
            {
                Ok(c) => {
                    self.context = ManuallyDrop::new(c);
                    Ok(self)
                }
                Err((_e, c)) => {
                    // TODO: Try to detect if the error is actually surface related.
                    let c = c.attempt_recovery()?;
                    match c.draw_next_frame(session, &mut *self.draw_pass) {
                        Ok(c) => {
                            self.context = ManuallyDrop::new(c);
                            Ok(self)
                        }
                        Err((e, _c)) => Err(e),
                    }
                }
            }
        }
    }

    /// Recreate the surface, and other derived components.
    /// This should be called when the window is resized.
    pub fn recreate_surface(mut self, session: &Session) -> Result<Renderer<DP>> {
        // Safety: If this fails at any point, the ManuallyDrop won't be touched again, as Renderer will be dropped.
        // Hence, we can always take from the ManuallyDrop
        unsafe {
            let ctx = ManuallyDrop::take(&mut self.context).recreate_surface()?;
            self.context = ManuallyDrop::new(ctx);

            let dp = ManuallyDrop::take(&mut self.draw_pass)
                .handle_surface_change(session, &mut self.context)?;
            self.draw_pass = ManuallyDrop::new(dp);
        }

        Ok(self)
    }

    pub fn get_aspect_ratio(&self) -> f32 {
        let e = self.context.properties().extent;
        e.width as f32 / e.height as f32
    }

    /// Get a reference to the renderer's context.
    pub fn context(&self) -> &RenderingContext {
        &self.context
    }
}
