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

use super::Q3BSPFile;
use crate::coords::CoordSystem;
use crate::helpers::{slice_to_u32, slice_to_vec3};
use crate::traits::vertices::*;
use crate::types::{ParseError, Result, RGBA};

/// The size of one vertex
const VERTEX_SIZE: usize = (4 * 3) + (2 * 2 * 4) + (4 * 3) + 4;

/// Parse a Vertices data from the data in a BSP file.
pub fn verts_from_data(data: &[u8]) -> Result<Box<[Vertex]>> {
    if data.len() % VERTEX_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let length = data.len() / VERTEX_SIZE;

    let mut vertices = Vec::with_capacity(length as usize);
    for n in 0..length {
        let offset = n * VERTEX_SIZE;
        let vertex = &data[offset..offset + VERTEX_SIZE];

        vertices.push(Vertex {
            position: slice_to_vec3(&vertex[0..12]),
            tex: TexCoord::from_bytes(&vertex[12..28].try_into().unwrap()),
            normal: slice_to_vec3(&vertex[28..40]),
            color: RGBA::from_slice(&vertex[40..44]),
        })
    }

    Ok(vertices.into_boxed_slice())
}

/// Parse the given data as a list of MeshVerts.
pub fn meshverts_from_data(data: &[u8]) -> Result<Box<[MeshVert]>> {
    if data.len() % 4 != 0 {
        return Err(ParseError::Invalid);
    }
    let length = data.len() / 4;

    let mut meshverts = Vec::with_capacity(length as usize);
    for n in 0..length {
        meshverts.push(slice_to_u32(&data[n * 4..(n + 1) * 4]))
    }

    Ok(meshverts.into_boxed_slice())
}

impl<T: CoordSystem> HasVertices<T> for Q3BSPFile<T> {
    type VerticesIter<'a> = std::slice::Iter<'a, Vertex>;

    fn vertices_iter(&self) -> Self::VerticesIter<'_> {
        self.vertices.iter()
    }

    fn get_vertex(&self, index: u32) -> &Vertex {
        &self.vertices[index as usize]
    }
}

impl<T: CoordSystem> HasMeshVerts<T> for Q3BSPFile<T> {
    type MeshVertsIter<'a> = std::slice::Iter<'a, MeshVert>;

    fn meshverts_iter(&self) -> Self::MeshVertsIter<'_> {
        self.meshverts.iter()
    }

    fn get_meshvert<'a>(&self, index: u32) -> MeshVert {
        self.meshverts[index as usize]
    }
}
