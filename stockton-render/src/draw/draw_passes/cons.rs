//! Code for using multiple draw passes in place of just one
//! Note that this can be extended to an arbitrary amount of draw passes.

use std::sync::{Arc, RwLock};

use super::{DrawPass, IntoDrawPass};
use crate::types::*;
use stockton_types::Session;

use anyhow::Result;

/// One draw pass, then another.
pub struct ConsDrawPass<A: DrawPass, B: DrawPass> {
    pub a: A,
    pub b: B,
}

impl<A: DrawPass, B: DrawPass> DrawPass for ConsDrawPass<A, B> {
    fn queue_draw(
        &mut self,
        session: &Session,
        img_view: &ImageViewT,
        cmd_buffer: &mut CommandBufferT,
    ) -> Result<()> {
        self.a.queue_draw(session, img_view, cmd_buffer)?;
        self.b.queue_draw(session, img_view, cmd_buffer)?;

        Ok(())
    }

    fn deactivate(self, device: &mut Arc<RwLock<DeviceT>>) -> Result<()> {
        self.a.deactivate(device)?;
        self.b.deactivate(device)
    }
}

impl<A: DrawPass, B: DrawPass, IA: IntoDrawPass<A>, IB: IntoDrawPass<B>>
    IntoDrawPass<ConsDrawPass<A, B>> for (IA, IB)
{
    fn init(
        self,
        session: &Session,
        adapter: &Adapter,
        device: Arc<RwLock<DeviceT>>,
        queue_negotiator: &mut crate::draw::queue_negotiator::QueueNegotiator,
        swapchain_properties: &crate::draw::target::SwapchainProperties,
    ) -> Result<ConsDrawPass<A, B>> {
        Ok(ConsDrawPass {
            a: self.0.init(
                session,
                adapter,
                device.clone(),
                queue_negotiator,
                swapchain_properties,
            )?,
            b: self.1.init(
                session,
                adapter,
                device,
                queue_negotiator,
                swapchain_properties,
            )?,
        })
    }

    fn find_aux_queues<'a>(
        adapter: &'a Adapter,
        queue_negotiator: &mut crate::draw::queue_negotiator::QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>> {
        let mut v = IA::find_aux_queues(adapter, queue_negotiator)?;
        v.extend(IB::find_aux_queues(adapter, queue_negotiator)?);
        Ok(v)
    }
}
