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

use crate::types::RGB;

#[derive(Debug, Clone, Copy)]
pub struct LightVol {
    pub ambient: RGB,
    pub directional: RGB,
    pub dir: [u8; 2],
}

pub trait HasLightVols {
    type LightVolsIter<'a>: Iterator<Item = &'a LightVol>;

    fn lightvols_iter(&self) -> Self::LightVolsIter<'_>;
    fn get_lightvol(&self, index: u32) -> &LightVol;
}
