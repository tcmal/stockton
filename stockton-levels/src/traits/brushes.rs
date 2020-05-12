// Copyright (C) 2019 Oscar Shrimpton
//
// This file is part of stockton-bsp.
//
// stockton-bsp is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// stockton-bsp is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with stockton-bsp.  If not, see <http://www.gnu.org/licenses/>.

//! Parses the brushes & brushsides lumps from a bsp file

use super::HasPlanes;

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

pub trait HasBrushes: HasPlanes {
    type BrushesIter<'a>: Iterator<Item = &'a Brush>;

    fn brushes_iter<'a>(&'a self) -> Self::BrushesIter<'a>;
    fn get_brush<'a>(&'a self, index: u32) -> &'a Brush;
}
