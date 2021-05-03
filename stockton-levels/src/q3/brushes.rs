//! Parses the brushes & brushsides lumps from a bsp file

/// The size of one brush record.
const BRUSH_SIZE: usize = 4 * 3;

/// The size of one brushsize record
const SIDE_SIZE: usize = 4 * 2;

use super::Q3BspFile;
use crate::coords::CoordSystem;
use crate::helpers::slice_to_i32;
use crate::traits::brushes::*;
use crate::types::{ParseError, Result};

/// Parse the brushes & brushsides lump from a bsp file.
pub fn from_data(
    brushes_data: &[u8],
    sides_data: &[u8],
    n_textures: u32,
    n_planes: u32,
) -> Result<Box<[Brush]>> {
    if brushes_data.len() % BRUSH_SIZE != 0 || sides_data.len() % SIDE_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let length = brushes_data.len() / BRUSH_SIZE;

    let mut brushes = Vec::with_capacity(length as usize);
    for n in 0..length {
        let offset = n * BRUSH_SIZE;
        let brush = &brushes_data[offset..offset + BRUSH_SIZE];

        let texture_idx = slice_to_i32(&brush[8..12]) as usize;
        if texture_idx >= n_textures as usize {
            return Err(ParseError::Invalid);
        }

        brushes.push(Brush {
            sides: get_sides(
                sides_data,
                slice_to_i32(&brush[0..4]),
                slice_to_i32(&brush[4..8]),
                n_textures as usize,
                n_planes as usize,
            )?,
            texture_idx,
        });
    }

    Ok(brushes.into_boxed_slice())
}

/// Internal function to get the relevant brushsides for a brush from the data in the brush lump.
fn get_sides(
    sides_data: &[u8],
    start: i32,
    length: i32,
    n_textures: usize,
    n_planes: usize,
) -> Result<Box<[BrushSide]>> {
    let mut sides = Vec::with_capacity(length as usize);

    if length > 0 {
        for n in start..start + length {
            let offset = n as usize * SIDE_SIZE;
            let brush = &sides_data[offset..offset + SIDE_SIZE];

            let plane_idx = slice_to_i32(&brush[0..4]) as usize;
            if plane_idx / 2 >= n_planes {
                return Err(ParseError::Invalid);
            }

            let is_opposing = plane_idx % 2 != 0;

            let texture_idx = slice_to_i32(&brush[4..8]) as usize;
            if texture_idx >= n_textures {
                return Err(ParseError::Invalid);
            }

            sides.push(BrushSide {
                plane_idx,
                texture_idx,
                is_opposing,
            });
        }
    }

    Ok(sides.into_boxed_slice())
}

impl<T: CoordSystem> HasBrushes<T> for Q3BspFile<T> {
    type BrushesIter<'a> = std::slice::Iter<'a, Brush>;

    fn brushes_iter(&self) -> Self::BrushesIter<'_> {
        self.brushes.iter()
    }

    fn get_brush(&self, index: u32) -> &Brush {
        &self.brushes[index as usize]
    }
}
