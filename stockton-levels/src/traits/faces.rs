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

use na::{Vector2, Vector3};
use std::ops::Range;

use super::{HasEffects, HasLightMaps, HasMeshVerts, HasTextures};
use crate::coords::CoordSystem;

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

pub trait HasFaces<S: CoordSystem>:
    HasTextures + HasEffects<S> + HasLightMaps + HasMeshVerts<S>
{
    type FacesIter<'a>: Iterator<Item = &'a Face>;

    fn faces_iter(&self) -> Self::FacesIter<'_>;
    fn faces_len(&self) -> u32;
    fn get_face(&self, index: u32) -> &Face;
}
