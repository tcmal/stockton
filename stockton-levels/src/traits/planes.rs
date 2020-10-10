// Copyright (C) 2019 Oscar Shrimpton
//
// This file is part of stockton-bsp.
//
// rust-bsp is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-bsp is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-bsp.  If not, see <http://www.gnu.org/licenses/>.

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
