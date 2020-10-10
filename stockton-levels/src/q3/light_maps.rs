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

use super::Q3BSPFile;
use crate::coords::CoordSystem;
use crate::traits::light_maps::*;
use crate::types::{ParseError, Result, RGB};

/// The size of one LightMap
const LIGHTMAP_SIZE: usize = 128 * 128 * 3;

/// Parse the LightMap data from a bsp file.
pub fn from_data(data: &[u8]) -> Result<Box<[LightMap]>> {
    if data.len() % LIGHTMAP_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let length = data.len() / LIGHTMAP_SIZE;

    let mut maps = Vec::with_capacity(length as usize);
    for n in 0..length {
        let raw = &data[n * LIGHTMAP_SIZE..(n + 1) * LIGHTMAP_SIZE];
        let mut map: [[RGB; 128]; 128] = [[RGB::white(); 128]; 128];

        for (x, outer) in map.iter_mut().enumerate() {
            for (y, inner) in outer.iter_mut().enumerate() {
                let offset = (x * 128 * 3) + (y * 3);
                *inner = RGB::from_slice(&raw[offset..offset + 3]);
            }
        }
        maps.push(LightMap { map })
    }

    Ok(maps.into_boxed_slice())
}

impl<T: CoordSystem> HasLightMaps for Q3BSPFile<T> {
    type LightMapsIter<'a> = std::slice::Iter<'a, LightMap>;

    fn lightmaps_iter(&self) -> Self::LightMapsIter<'_> {
        self.light_maps.iter()
    }

    fn get_lightmap(&self, index: u32) -> &LightMap {
        &self.light_maps[index as usize]
    }
}
