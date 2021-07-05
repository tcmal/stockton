//! Code for using multiple draw passes in place of just one
//! Note that this can be extended to an arbitrary amount of draw passes.

use super::{DrawPass, DrawPassInput};
use crate::{draw::queue_negotiator::QueueNegotiator, types::*};
use anyhow::Result;

/// One draw pass, then another.
struct ConsDrawPass<A: DrawPass, B: DrawPass> {
    a: A,
    b: B,
}

impl<A: DrawPass, B: DrawPass> DrawPass for ConsDrawPass<A, B> {
    type Input = ConsDrawPassInput<A::Input, B::Input>;

    fn queue_draw(&self, input: &Self::Input, cmd_buffer: &mut CommandBufferT) -> Result<()> {
        self.a.queue_draw(&input.a, cmd_buffer)?;
        self.b.queue_draw(&input.b, cmd_buffer)?;

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

/// Input for a ConsDrawPass.
struct ConsDrawPassInput<A, B> {
    pub a: A,
    pub b: B,
}

impl<A: DrawPassInput, B: DrawPassInput> DrawPassInput for ConsDrawPassInput<A, B> {}

/// A draw pass that does nothing. Can be used at the end of sequences if there's an odd number of draw passes.
struct NilDrawPass;

impl DrawPass for NilDrawPass {
    type Input = NilDrawPassInput;

    fn queue_draw(&self, _input: &Self::Input, _cmd_buffer: &mut CommandBufferT) -> Result<()> {
        Ok(())
    }

    fn find_aux_queues<'a>(
        _adapter: &'a Adapter,
        _queue_negotiator: &mut QueueNegotiator,
    ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>> {
        Ok(vec![])
    }
}

/// Input for a NilDrawPass.
struct NilDrawPassInput;

impl DrawPassInput for NilDrawPassInput {}
