//! Parses the brushes & brushsides lumps from a bsp file

use super::HasPlanes;
use crate::coords::CoordSystem;

/// One brush record. Used for collision detection.
/// "Each brush describes a convex volume as defined by its surrounding surfaces."
#[derive(Debug, Clone, PartialEq)]
pub struct Brush {
    pub sides: Box<[BrushSide]>,
    pub texture_idx: usize,
}

/// Bounding surface for brush.
#[derive(Debug, Clone, PartialEq)]
pub struct BrushSide {
    pub plane_idx: usize,
    pub texture_idx: usize,
    pub is_opposing: bool,
}

pub trait HasBrushes<S: CoordSystem>: HasPlanes<S> {
    type BrushesIter<'a>: Iterator<Item = &'a Brush>;

    fn brushes_iter(&self) -> Self::BrushesIter<'_>;
    fn get_brush(&self, index: u32) -> &Brush;
}
