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
