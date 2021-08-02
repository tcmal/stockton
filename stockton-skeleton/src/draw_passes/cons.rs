//! Code for using multiple draw passes in place of just one
//! Note that this can be extended to an arbitrary amount of draw passes.

use super::{Beginning, DrawPass, End, IntoDrawPass, Middle, Singular};
use crate::{context::RenderingContext, queue_negotiator::QueueNegotiator, types::*};
use stockton_types::Session;

use anyhow::Result;

/// One draw pass, then another.
pub struct ConsDrawPass<A, B> {
    pub a: A,
    pub b: B,
}

macro_rules! cons_shared_impl {
    () => {
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
        fn deactivate(self, context: &mut RenderingContext) -> Result<()> {
            self.a.deactivate(context)?;
            self.b.deactivate(context)
        }

        fn handle_surface_change(
            &mut self,
            session: &Session,
            context: &mut RenderingContext,
        ) -> Result<()> {
            self.a.handle_surface_change(session, context)?;
            self.b.handle_surface_change(session, context)
        }
    };
}

impl<A, B> DrawPass<Singular> for ConsDrawPass<A, B>
where
    A: DrawPass<Beginning>,
    B: DrawPass<End>,
{
    cons_shared_impl! {}
}

impl<A, B> DrawPass<End> for ConsDrawPass<A, B>
where
    A: DrawPass<Middle>,
    B: DrawPass<End>,
{
    cons_shared_impl! {}
}

macro_rules! into_shared_impl {
    () => {
        fn init(
            self,
            session: &mut Session,
            context: &mut RenderingContext,
        ) -> Result<ConsDrawPass<A, B>> {
            Ok(ConsDrawPass {
                a: self.0.init(session, context)?,
                b: self.1.init(session, context)?,
            })
        }

        fn find_aux_queues<'a>(
            adapter: &'a Adapter,
            queue_negotiator: &mut QueueNegotiator,
        ) -> Result<Vec<(&'a QueueFamilyT, Vec<f32>)>> {
            let mut v = IA::find_aux_queues(adapter, queue_negotiator)?;
            v.extend(IB::find_aux_queues(adapter, queue_negotiator)?);
            Ok(v)
        }
    };
}

impl<A, B, IA, IB> IntoDrawPass<ConsDrawPass<A, B>, Singular> for (IA, IB)
where
    A: DrawPass<Beginning>,
    B: DrawPass<End>,
    IA: IntoDrawPass<A, Beginning>,
    IB: IntoDrawPass<B, End>,
{
    into_shared_impl! {}
}

impl<A, B, IA, IB> IntoDrawPass<ConsDrawPass<A, B>, End> for (IA, IB)
where
    A: DrawPass<Middle>,
    B: DrawPass<End>,
    IA: IntoDrawPass<A, Middle>,
    IB: IntoDrawPass<B, End>,
{
    into_shared_impl! {}
}
