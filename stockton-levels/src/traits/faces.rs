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
