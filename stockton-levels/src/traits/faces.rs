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

use std::ops::Range;
use na::{Vector2, Vector3};

use super::{HasEffects, HasTextures, HasLightMaps, HasMeshVerts};

#[derive(Debug, Clone, Copy, PartialEq)]
#[repr(i32)]
pub enum FaceType {
    Polygon = 1,
    Patch = 2,
    Mesh = 3,
    Billboard = 4,
}

#[derive(Debug, Clone, PartialEq)]
pub struct Face {
    pub face_type: FaceType,
    pub texture_idx: u32,
    pub effect_idx: Option<u32>,
    pub lightmap_idx: Option<u32>,
    pub vertices_idx: Range<u32>,
    pub meshverts_idx: Range<u32>,

    pub map_start: Vector2<u32>,
    pub map_size: Vector2<u32>,
    pub map_origin: Vector3<f32>,
    pub map_vecs: [Vector3<f32>; 2],
    
    pub normal: Vector3<f32>,
    pub size: Vector2<u32>,
}

pub trait HasFaces: HasTextures + HasEffects + HasLightMaps + HasMeshVerts {
    type FacesIter<'a>: Iterator<Item = &'a Face>;

    fn faces_iter<'a>(&'a self) -> Self::FacesIter<'a>;
    fn faces_len(&self) -> u32;
    fn get_face<'a>(&'a self, index: u32) -> &'a Face;
}
