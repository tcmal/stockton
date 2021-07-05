//! Code for using multiple draw passes in place of just one
//! Note that this can be extended to an arbitrary amount of draw passes.

use super::DrawPass;
use crate::{draw::queue_negotiator::QueueNegotiator, types::*};
use stockton_types::Session;

use anyhow::Result;

/// One draw pass, then another.
pub struct ConsDrawPass<A: DrawPass, B: DrawPass> {
    a: A,
    b: B,
}

impl<A: DrawPass, B: DrawPass> DrawPass for ConsDrawPass<A, B> {
    fn queue_draw(&self, session: &Session, cmd_buffer: &mut CommandBufferT) -> Result<()> {
        self.a.queue_draw(&session, cmd_buffer)?;
        self.b.queue_draw(&session, cmd_buffer)?;

        Ok(())
    }

    fn find_aux_queues<'a>(
        adapter: &'a Adapter,
        queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>> {
        let mut vec = Vec::new();

        vec.extend(A::find_aux_queues(adapter, queue_negotiator)?);
        vec.extend(B::find_aux_queues(adapter, queue_negotiator)?);

        Ok(vec)
    }
}

/// A draw pass that does nothing. Can be used at the end of sequences if there's an odd number of draw passes.
pub struct NilDrawPass;

impl DrawPass for NilDrawPass {

    fn queue_draw(&self, _input: &Session, _cmd_buffer: &mut CommandBufferT) -> Result<()> {
        Ok(())
    }

    fn find_aux_queues<'a>(
        _adapter: &'a Adapter,
        _queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>> {
        Ok(vec![])
    }
}