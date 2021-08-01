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

/// A draw pass that does nothing. Can be used at the end of sequences if there's an odd number of draw passes.
pub struct NilDrawPass;

impl DrawPass for NilDrawPass {
    fn queue_draw(
        &mut self,
        _input: &Session,
        _img_view: &ImageViewT,
        _cmd_buffer: &mut CommandBufferT,
    ) -> Result<()> {
        Ok(())
    }

    fn deactivate(self, _device: &mut Arc<RwLock<DeviceT>>) -> Result<()> {
        Ok(())
    }
}
