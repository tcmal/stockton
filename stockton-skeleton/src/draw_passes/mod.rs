//! Traits and common draw passes.
use std::ops::Range;

use crate::{context::RenderingContext, queue_negotiator::QueueFamilyNegotiator, types::*, session::Session};
use hal::{
    image::Layout,
    pass::{AttachmentLoadOp, AttachmentOps, AttachmentStoreOp},
};

use anyhow::Result;

mod cons;
pub mod util;

pub use cons::ConsDrawPass;

/// One of several 'passes' that draw on each frame.
pub trait DrawPass<P: PassPosition> {
    /// Queue any necessary draw commands to cmd_buffer
    /// This should assume the command buffer isn't in the middle of a renderpass, and should leave it as such.
    fn queue_draw(
        &mut self,
        session: &Session,
        img_view: &ImageViewT,
        cmd_buffer: &mut CommandBufferT,
    ) -> Result<()>;

    /// Called just after the surface changes (probably a resize).
    /// This takes ownership and returns itself to ensure that the `DrawPass` is not called again if it fails.
    /// This means you should deactivate as much as possible in case of an error.
    fn handle_surface_change(
        self,
        session: &Session,
        context: &mut RenderingContext,
    ) -> Result<Self>
    where
        Self: Sized;

    /// Deactivate any vulkan parts that need to be deactivated
    fn deactivate(self, context: &mut RenderingContext) -> Result<()>;
}

/// A type that can be made into a specific draw pass type.
/// This allows extra data to be used in initialisation without the Renderer needing to worry about it.
pub trait IntoDrawPass<T: DrawPass<P>, P: PassPosition> {
    fn init(self, session: &mut Session, context: &mut RenderingContext) -> Result<T>;

    /// This function should ask the queue negotatior to find families for any auxilary operations this draw pass needs to perform
    /// For example, .find(&TexLoadQueue)
    fn find_aux_queues(
        adapter: &Adapter,
        queue_negotiator: &mut QueueFamilyNegotiator,
    ) -> Result<()>;
}

/// Used so that draw passes can determine what state shared resources are in and how they should be left.
pub trait PassPosition: private::Sealed {
    /// The layout the image is in going in.
    fn layout_in() -> Layout;

    /// The layout the image should be once this drawpass is completed
    fn layout_out() -> Layout;

    /// Has the layout already been cleared this frame
    fn is_cleared() -> bool;

    /// Convenience function to get a range from layout_in() to layout_out()
    fn layout_as_range() -> Range<Layout> {
        Self::layout_in()..Self::layout_out()
    }

    /// Convenience function to get the attachment ops that should be used when loading the image attachment.
    fn attachment_ops() -> AttachmentOps {
        match Self::is_cleared() {
            true => AttachmentOps::new(AttachmentLoadOp::Load, AttachmentStoreOp::Store),
            false => AttachmentOps::new(AttachmentLoadOp::Clear, AttachmentStoreOp::Store),
        }
    }
}

/// Pass is at the beginning of the list
pub struct Beginning;
impl PassPosition for Beginning {
    fn layout_in() -> Layout {
        Layout::Undefined
    }

    fn layout_out() -> Layout {
        Layout::ColorAttachmentOptimal
    }

    fn is_cleared() -> bool {
        false
    }
}

/// Pass is in the middle of the list
pub struct Middle;
impl PassPosition for Middle {
    fn layout_in() -> Layout {
        Layout::ColorAttachmentOptimal
    }

    fn layout_out() -> Layout {
        Layout::ColorAttachmentOptimal
    }

    fn is_cleared() -> bool {
        true
    }
}

/// Pass is at the end of the list
pub struct End;
impl PassPosition for End {
    fn layout_in() -> Layout {
        Layout::ColorAttachmentOptimal
    }

    fn layout_out() -> Layout {
        Layout::Present
    }

    fn is_cleared() -> bool {
        true
    }
}

/// Pass is the only draw pass being used
pub struct Singular;
impl PassPosition for Singular {
    fn layout_in() -> Layout {
        Layout::Undefined
    }

    fn layout_out() -> Layout {
        Layout::Present
    }

    fn is_cleared() -> bool {
        false
    }
}

mod private {
    use super::{Beginning, End, Middle, Singular};

    pub trait Sealed {}
    impl Sealed for Beginning {}
    impl Sealed for Middle {}
    impl Sealed for End {}
    impl Sealed for Singular {}
}
