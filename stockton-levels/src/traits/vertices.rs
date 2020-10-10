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

use crate::coords::CoordSystem;
use crate::helpers::slice_to_f32;
use crate::types::RGBA;
use na::Vector3;

/// A vertex, used to describe a face.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Vertex {
    pub position: Vector3<f32>,
    pub tex: TexCoord,
    pub normal: Vector3<f32>,
    pub color: RGBA,
}

/// Represents a TexCoord. 0 = surface, 1= lightmap.
/// This could also be written as [[f32; 2]; 2]
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct TexCoord {
    pub u: [f32; 2],
    pub v: [f32; 2],
}

impl TexCoord {
    /// Internal function. Converts a slice to a TexCoord.
    pub fn from_bytes(bytes: &[u8; 16]) -> TexCoord {
        TexCoord {
            u: [slice_to_f32(&bytes[0..4]), slice_to_f32(&bytes[8..12])],
            v: [slice_to_f32(&bytes[4..8]), slice_to_f32(&bytes[12..16])],
        }
    }
}

/// A vertex offset, used to describe generalised triangle meshes
pub type MeshVert = u32;

pub trait HasVertices<S: CoordSystem> {
    type VerticesIter<'a>: Iterator<Item = &'a Vertex>;

    fn vertices_iter(&self) -> Self::VerticesIter<'_>;
    fn get_vertex(&self, index: u32) -> &Vertex;
}

pub trait HasMeshVerts<S: CoordSystem>: HasVertices<S> {
    type MeshVertsIter<'a>: Iterator<Item = &'a MeshVert>;

    fn meshverts_iter(&self) -> Self::MeshVertsIter<'_>;
    fn get_meshvert(&self, index: u32) -> MeshVert;

    fn resolve_meshvert(&self, index: u32, base: u32) -> &Vertex {
        self.get_vertex(self.get_meshvert(index) + base)
    }
}
