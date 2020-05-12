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

use na::Vector3;
use std::ops::Range;

use super::{HasFaces, HasBrushes};

#[derive(Debug, Clone)]
pub struct Model {
    pub mins: Vector3<f32>,
    pub maxs: Vector3<f32>,
    pub faces_idx: Range<u32>,
    pub brushes_idx: Range<u32>,
}

pub trait HasModels: HasFaces + HasBrushes {
    type ModelsIter<'a>: Iterator<Item = &'a Model>;

    fn models_iter<'a>(&'a self) -> Self::ModelsIter<'a>;
    fn get_model<'a>(&'a self, index: u32) -> &'a Model;
}
