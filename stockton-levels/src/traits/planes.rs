use crate::coords::CoordSystem;
use na::Vector3;
use std::iter::Iterator;

/// The planes lump from a BSP file.
/// Found at lump index 2 in a q3 bsp.
#[derive(Debug, Clone)]
pub struct PlanesLump {
    pub planes: Box<[Plane]>,
}

/// Generic plane, referenced by nodes & brushsizes
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Plane {
    /// Plane normal
    pub normal: Vector3<f32>,

    /// Distance from origin to plane along normal
    pub dist: f32,
}

pub trait HasPlanes<S: CoordSystem> {
    type PlanesIter<'a>: Iterator<Item = &'a Plane>;

    fn planes_iter(&self) -> Self::PlanesIter<'_>;
    fn get_plane(&self, idx: u32) -> &Plane;
}
