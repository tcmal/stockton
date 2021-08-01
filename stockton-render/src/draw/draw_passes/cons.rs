//! Code for using multiple draw passes in place of just one
//! Note that this can be extended to an arbitrary amount of draw passes.

use std::sync::{Arc, RwLock};

use super::DrawPass;
use crate::types::*;
use stockton_types::Session;

use anyhow::Result;

/// One draw pass, then another.
pub struct ConsDrawPass<A: DrawPass, B: DrawPass> {
    a: A,
    b: B,
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
