//! Traits and common draw passes.

mod cons;
mod level;
use std::sync::{Arc, RwLock};

pub use level::LevelDrawPass;

use super::{queue_negotiator::QueueNegotiator, target::SwapchainProperties};
use crate::types::*;
use anyhow::Result;

/// Type can be used as input to a draw pass. This requires it being available from only the resources at draw time.
pub trait DrawPassInput {}

/// One of several 'passes' that draw on each frame.
pub trait DrawPass {
    /// Extra input required for this draw pass.
    type Input: DrawPassInput;

    /// Queue any necessary draw commands to cmd_buffer
    /// This should assume the command buffer isn't in the middle of a renderpass, and should leave it as such.
    fn queue_draw(&self, input: &Self::Input, cmd_buffer: &mut CommandBufferT) -> Result<()>;

    /// This function should ask the queue negotatior to find families for any auxilary operations this draw pass needs to perform
    /// For example, .find(&TexLoadQueue)
    /// It should return then call .family_spec for each queue type negotiated and return the results.
    fn find_aux_queues<'a>(
        adapter: &'a Adapter,
        queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>>;
}

/// A type that can be made into a specific draw pass type.
/// This allows extra data to be used in initialisation without the Renderer needing to worry about it.
pub trait IntoDrawPass<O: DrawPass> {
    fn init(
        self,
        device: Arc<RwLock<DeviceT>>,
        queue_negotiator: &mut QueueNegotiator,
        swapchain_properties: &SwapchainProperties,
    ) -> Result<O>;
}
