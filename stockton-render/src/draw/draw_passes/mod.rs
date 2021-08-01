//! Traits and common draw passes.
use super::{queue_negotiator::QueueNegotiator, target::SwapchainProperties};
use crate::types::*;
use stockton_types::Session;

use std::sync::{Arc, RwLock};

use anyhow::Result;

mod cons;
mod level;

pub use cons::{ConsDrawPass, NilDrawPass};
pub use level::LevelDrawPass;

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

    fn deactivate(self, device: &mut Arc<RwLock<DeviceT>>) -> Result<()>;
}

/// A type that can be made into a specific draw pass type.
/// This allows extra data to be used in initialisation without the Renderer needing to worry about it.
pub trait IntoDrawPass<O: DrawPass> {
    fn init(
        self,
        session: &Session,
        adapter: &Adapter,
        device: Arc<RwLock<DeviceT>>,
        queue_negotiator: &mut QueueNegotiator,
        swapchain_properties: &SwapchainProperties,
    ) -> Result<O>;

    /// This function should ask the queue negotatior to find families for any auxilary operations this draw pass needs to perform
    /// For example, .find(&TexLoadQueue)
    /// It should return then call .family_spec for each queue type negotiated and return the results.
    fn find_aux_queues<'a>(
        adapter: &'a Adapter,
        queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>>;
}
