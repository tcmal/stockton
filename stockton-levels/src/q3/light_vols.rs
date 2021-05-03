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

use std::convert::TryInto;

use super::Q3BspFile;
use crate::coords::CoordSystem;
use crate::traits::light_vols::*;
use crate::types::{ParseError, Result, Rgb};

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
            ambient: Rgb::from_slice(&data[0..3]),
            directional: Rgb::from_slice(&data[3..6]),
            dir: data[6..8].try_into().unwrap(),
        });
    }

    Ok(vols.into_boxed_slice())
}

impl<T: CoordSystem> HasLightVols for Q3BspFile<T> {
    type LightVolsIter<'a> = std::slice::Iter<'a, LightVol>;

    fn lightvols_iter(&self) -> Self::LightVolsIter<'_> {
        self.light_vols.iter()
    }

    fn get_lightvol(&self, index: u32) -> &LightVol {
        &self.light_vols[index as usize]
    }
}
