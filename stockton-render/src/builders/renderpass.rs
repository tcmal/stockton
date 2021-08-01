use crate::types::*;

use std::iter::{empty, once};

use anyhow::Result;
use hal::pass::{Attachment, AttachmentRef, SubpassDesc};

#[derive(Debug, Clone)]
pub struct RenderpassSpec {
    pub colors: Vec<Attachment>,
    pub depth: Option<Attachment>,
    pub inputs: Vec<Attachment>,
    pub resolves: Vec<Attachment>,
    pub preserves: Vec<Attachment>,
}

impl RenderpassSpec {
    pub fn build_renderpass(self, device: &mut DeviceT) -> Result<RenderPassT> {
        let mut next_offset = 0;

        let colors: Vec<AttachmentRef> = self
            .colors
            .iter()
            .enumerate()
            .map(|(i, a)| (next_offset + i, a.layouts.end))
            .collect();
        next_offset = colors.len();

        let depth_stencil = self.depth.as_ref().map(|x| (next_offset, x.layouts.end));
        if depth_stencil.is_some() {
            next_offset += 1;
        }

        let inputs: Vec<AttachmentRef> = self
            .inputs
            .iter()
            .enumerate()
            .map(|(i, a)| (next_offset + i, a.layouts.end))
            .collect();
        next_offset += inputs.len();

        let resolves: Vec<AttachmentRef> = self
            .resolves
            .iter()
            .enumerate()
            .map(|(i, a)| (next_offset + i, a.layouts.end))
            .collect();
        next_offset += resolves.len();

        let preserves: Vec<usize> = self
            .preserves
            .iter()
            .enumerate()
            .map(|(i, _a)| next_offset + i)
            .collect();

        let sp_desc = SubpassDesc {
            colors: colors.as_slice(),
            depth_stencil: depth_stencil.as_ref(),
            inputs: inputs.as_slice(),
            resolves: resolves.as_slice(),
            preserves: preserves.as_slice(),
        };

        let all_attachments = self
            .colors
            .into_iter()
            .chain(self.depth.into_iter())
            .chain(self.inputs.into_iter())
            .chain(self.resolves.into_iter())
            .chain(self.preserves.into_iter());

        Ok(unsafe { device.create_render_pass(all_attachments, once(sp_desc), empty())? })
    }
}
