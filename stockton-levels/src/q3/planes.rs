const PLANE_SIZE: usize = (4 * 3) + 4;

use super::Q3BspFile;
use crate::coords::CoordSystem;
use crate::helpers::{slice_to_f32, slice_to_vec3};
use crate::traits::planes::*;
use crate::types::{ParseError, Result};

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

impl<T: CoordSystem> HasPlanes<T> for Q3BspFile<T> {
    type PlanesIter<'a> = std::slice::Iter<'a, Plane>;

    fn planes_iter(&self) -> Self::PlanesIter<'_> {
        self.planes.iter()
    }

    fn get_plane(&self, idx: u32) -> &Plane {
        &self.planes[idx as usize]
    }
}
