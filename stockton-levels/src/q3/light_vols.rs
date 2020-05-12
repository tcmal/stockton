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

use std::convert::TryInto;

use crate::types::{Result, ParseError, RGB};
use crate::traits::light_vols::*;
use super::Q3BSPFile;

const VOL_LENGTH: usize = (3 * 2) + 2;

pub fn from_data(data: &[u8]) -> Result<Box<[LightVol]>> {
    if data.len() % VOL_LENGTH != 0 {
        return Err(ParseError::Invalid);
    }
    let length = data.len() / VOL_LENGTH;

    let mut vols = Vec::with_capacity(length);
    for n in 0..length {
        let data = &data[n * VOL_LENGTH..(n + 1) * VOL_LENGTH];
        vols.push(LightVol {
            ambient: RGB::from_slice(&data[0..3]),
            directional: RGB::from_slice(&data[3..6]),
            dir: data[6..8].try_into().unwrap(),
        });
    }

    Ok(vols.into_boxed_slice())
}


impl HasLightVols for Q3BSPFile {
    type LightVolsIter<'a> = std::slice::Iter<'a, LightVol>;

    fn lightvols_iter<'a>(&'a self) -> Self::LightVolsIter<'a> {
        self.light_vols.iter()
    }

    fn get_lightvol<'a>(&'a self, index: u32) -> &'a LightVol {
        &self.light_vols[index as usize]
    }
}
