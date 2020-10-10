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

use super::HasBrushes;
use crate::coords::CoordSystem;

/// One effect definition
#[derive(Debug, Clone, PartialEq)]
pub struct Effect {
    /// The name of the effect - always 64 characters long
    pub name: String,

    /// The brush used for this effect
    pub brush_idx: u32, // todo: unknown: i32
}

pub trait HasEffects<S: CoordSystem>: HasBrushes<S> {
    type EffectsIter<'a>: Iterator<Item = &'a Effect>;

    fn effects_iter(&self) -> Self::EffectsIter<'_>;
    fn get_effect(&self, index: u32) -> &Effect;
}
