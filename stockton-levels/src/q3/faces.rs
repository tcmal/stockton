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

use crate::helpers::{slice_to_i32, slice_to_vec2i, slice_to_vec3};
use crate::types::{Result, ParseError};
use na::Vector3;
use crate::traits::faces::*;
use super::Q3BSPFile;

const FACE_SIZE: usize = (4 * 8) + (4 * 2) + (4 * 2) + (4 * 3) + ((4 * 2) * 3) + (4 * 3) + (4 * 2);


pub fn from_data(
    data: &[u8],
    n_textures: u32,
    n_effects: u32,
    n_vertices: u32,
    n_meshverts: u32,
    n_lightmaps: u32,
) -> Result<Box<[Face]>> {
    if data.len() % FACE_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let length = data.len() / FACE_SIZE;

    let mut faces = Vec::with_capacity(length);
    for n in 0..length {
        faces.push(face_from_slice(
            &data[n * FACE_SIZE..(n + 1) * FACE_SIZE],
            n_textures as usize,
            n_effects as usize,
            n_vertices as usize,
            n_meshverts as usize,
            n_lightmaps as usize,
        )?);
    }

    Ok(faces.into_boxed_slice())
}


fn face_from_slice(
    data: &[u8],
    n_textures: usize,
    n_effects: usize,
    n_vertices: usize,
    n_meshverts: usize,
    n_lightmaps: usize,
) -> Result<Face> {
    if data.len() != FACE_SIZE {
        panic!("tried to call face.from_slice with invalid slice size");
    }

    // texture
    let texture_idx = slice_to_i32(&data[0..4]) as usize;
    if texture_idx >= n_textures {
        return Err(ParseError::Invalid);
    }

    // effects
    let effect_idx = slice_to_i32(&data[4..8]) as usize;
    let effect_idx = if effect_idx < 0xffffffff {
        if effect_idx >= n_effects {
            return Err(ParseError::Invalid);
        }

        Some(effect_idx)
    } else {
        None
    };

    // face type
    // TODO
    let face_type: FaceType = unsafe { ::std::mem::transmute(slice_to_i32(&data[8..12])) };

    // vertices
    let vertex_offset = slice_to_i32(&data[12..16]) as usize;
    let vertex_n = slice_to_i32(&data[16..20]) as usize;
    if (vertex_offset + vertex_n) > n_vertices {
        return Err(ParseError::Invalid);
    }

    let vertices_idx = vertex_offset..vertex_offset + vertex_n;

    // meshverts
    let meshverts_offset = slice_to_i32(&data[20..24]) as usize;
    let meshverts_n = slice_to_i32(&data[24..28]) as usize;
    if (meshverts_offset + meshverts_n) > n_meshverts {
        return Err(ParseError::Invalid);
    }

    let meshverts_idx = meshverts_offset..meshverts_offset + meshverts_n;

    // lightmap
    let lightmap_idx = slice_to_i32(&data[28..32]) as usize;
    let lightmap_idx = if lightmap_idx < 0xffffffff {
        if lightmap_idx >= n_lightmaps {
            return Err(ParseError::Invalid);
        }

        Some(lightmap_idx)
    } else {
        None
    };

    // map properties
    let map_start = slice_to_vec2i(&data[32..40]);
    let map_size = slice_to_vec2i(&data[40..48]);
    let map_origin = slice_to_vec3(&data[48..60]);

    // map_vecs
    let mut map_vecs = [Vector3::new(0.0, 0.0, 0.0); 2];
    for n in 0..2 {
        let offset = 60 + (n * 3 * 4);
        map_vecs[n] = slice_to_vec3(&data[offset..offset + 12]);
    }

    // normal & size
    let normal = slice_to_vec3(&data[84..96]);
    let size = slice_to_vec2i(&data[96..104]);

    Ok(Face {
        face_type,
        texture_idx,
        effect_idx,
        vertices_idx,
        meshverts_idx,
        lightmap_idx,
        map_start,
        map_size,
        map_origin,
        map_vecs,
        normal,
        size,
    })
}


impl<'a> HasFaces<'a> for Q3BSPFile {
    type FacesIter = std::slice::Iter<'a, Face>;

    fn faces_iter(&'a self) -> Self::FacesIter {
        self.faces.iter()
    }

    fn get_face(&'a self, index: u32) -> &'a Face {
        &self.faces[index as usize]
    }
}
