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

use super::Q3BspFile;
use crate::coords::CoordSystem;
use crate::traits::light_maps::*;
use crate::types::{ParseError, Result, Rgb};

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
        let mut map: [[Rgb; 128]; 128] = [[Rgb::white(); 128]; 128];

        for (x, outer) in map.iter_mut().enumerate() {
            for (y, inner) in outer.iter_mut().enumerate() {
                let offset = (x * 128 * 3) + (y * 3);
                *inner = Rgb::from_slice(&raw[offset..offset + 3]);
            }
        }
        maps.push(LightMap { map })
    }

    Ok(maps.into_boxed_slice())
}

impl<T: CoordSystem> HasLightMaps for Q3BspFile<T> {
    type LightMapsIter<'a> = std::slice::Iter<'a, LightMap>;

    fn lightmaps_iter(&self) -> Self::LightMapsIter<'_> {
        self.light_maps.iter()
    }

    fn get_lightmap(&self, index: u32) -> &LightMap {
        &self.light_maps[index as usize]
    }
}
