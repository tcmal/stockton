/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

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
