//! Traits and common draw passes.
use super::{queue_negotiator::QueueNegotiator, RenderingContext};
use crate::types::*;
use stockton_types::Session;

use anyhow::Result;

mod cons;
pub mod util;

pub use cons::ConsDrawPass;

/// One of several 'passes' that draw on each frame.
pub trait DrawPass {
    /// Queue any necessary draw commands to cmd_buffer
    /// This should assume the command buffer isn't in the middle of a renderpass, and should leave it as such.
    fn queue_draw(
        &mut self,
        session: &Session,
        img_view: &ImageViewT,
        cmd_buffer: &mut CommandBufferT,
    ) -> Result<()>;

    /// Called just after the surface changes (probably a resize).
    fn handle_surface_change(
        &mut self,
        session: &Session,
        context: &mut RenderingContext,
    ) -> Result<()>;

    /// Deactivate any vulkan parts that need to be deactivated
    fn deactivate(self, context: &mut RenderingContext) -> Result<()>;
}

/// A type that can be made into a specific draw pass type.
/// This allows extra data to be used in initialisation without the Renderer needing to worry about it.
pub trait IntoDrawPass<T: DrawPass> {
    fn init(self, session: &mut Session, context: &mut RenderingContext) -> Result<T>;

    /// This function should ask the queue negotatior to find families for any auxilary operations this draw pass needs to perform
    /// For example, .find(&TexLoadQueue)
    /// It should return then call .family_spec for each queue type negotiated and return the results.
    fn find_aux_queues<'a>(
        adapter: &'a Adapter,
        queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>>;
}
