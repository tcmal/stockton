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
