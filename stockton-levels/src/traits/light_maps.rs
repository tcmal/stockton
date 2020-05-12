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

use std::fmt;

use crate::types::RGB;

/// Stores light map textures that help make surface lighting more realistic
#[derive(Clone)]
pub struct LightMap {
    pub map: [[RGB; 128]; 128],
}

impl PartialEq for LightMap {
    fn eq(&self, other: &LightMap) -> bool {
        for x in 0..128 {
            for y in 0..128 {
                if self.map[x][y] != other.map[x][y] {
                    return false;
                }
            }
        }
        true
    }
}

impl fmt::Debug for LightMap {
    // rust can't derive debug for 3d arrays so done manually
    // \_( )_/
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LightMap {{ map: [")?;
        for c in self.map.iter() {
            write!(f, "[")?;
            for x in c.iter() {
                write!(f, "{:?}, ", x)?;
            }
            write!(f, "], ")?;
        }
        write!(f, "}}")
    }
}

pub trait HasLightMaps {
    type LightMapsIter<'a>: Iterator<Item = &'a LightMap>;

    fn lightmaps_iter<'a>(&'a self) -> Self::LightMapsIter<'a>;
    fn get_lightmap<'a>(&'a self, index: u32) -> &'a LightMap;
}