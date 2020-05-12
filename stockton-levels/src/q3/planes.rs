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

const PLANE_SIZE: usize = (4 * 3) + 4;

use crate::helpers::{slice_to_f32, slice_to_vec3};
use crate::types::{Result, ParseError};
use crate::traits::planes::*;
use super::Q3BSPFile;
use crate::coords::CoordSystem;

/// Parse a lump of planes.
/// A lump is (data length / plane size) planes long
pub fn from_data(data: &[u8]) -> Result<Box<[Plane]>> {

    let length = data.len() / PLANE_SIZE;
    if data.is_empty() || data.len() % PLANE_SIZE != 0 || length % 2 != 0 {
        return Err(ParseError::Invalid);
    }


    let mut planes = Vec::with_capacity(length / 2);
    for n in 0..length {
        let offset = n * PLANE_SIZE;
        let plane = &data[offset..offset + PLANE_SIZE];
        planes.push(Plane {
            normal: slice_to_vec3(&plane[0..12]),
            dist: slice_to_f32(&plane[12..16]),
        });
    }

    Ok(planes.into_boxed_slice())
}

impl<T: CoordSystem> HasPlanes<T> for Q3BSPFile<T> {
    type PlanesIter<'a> = std::slice::Iter<'a, Plane>;

    fn planes_iter<'a>(&'a self) -> Self::PlanesIter<'a> {
        self.planes.iter()
    }

    fn get_plane<'a>(&'a self, idx: u32) -> &'a Plane {
        &self.planes[idx as usize]
    }
}