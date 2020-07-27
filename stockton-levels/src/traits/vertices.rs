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

use crate::helpers::{slice_to_f32};
use crate::coords::CoordSystem;
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

    fn vertices_iter<'a>(&'a self) -> Self::VerticesIter<'a>;
    fn get_vertex<'a>(&'a self, index: u32) -> &'a Vertex;
}

pub trait HasMeshVerts<S: CoordSystem>: HasVertices<S> {
    type MeshVertsIter<'a>: Iterator<Item = &'a MeshVert>;

    fn meshverts_iter<'a>(&'a self) -> Self::MeshVertsIter<'a>;
    fn get_meshvert(&self, index: u32) -> MeshVert;

    fn resolve_meshvert<'a>(&'a self, index: u32, base: u32) -> &'a Vertex {
        self.get_vertex(self.get_meshvert(index) + base)
    }
}