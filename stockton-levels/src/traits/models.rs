use na::Vector3;
use std::ops::Range;

use super::{HasBrushes, HasFaces};
use crate::coords::CoordSystem;

#[derive(Debug, Clone)]
pub struct Model {
    pub mins: Vector3<f32>,
    pub maxs: Vector3<f32>,
    pub faces_idx: Range<u32>,
    pub brushes_idx: Range<u32>,
}

pub trait HasModels<S: CoordSystem>: HasFaces<S> + HasBrushes<S> {
    type ModelsIter<'a>: Iterator<Item = &'a Model>;

    fn models_iter(&self) -> Self::ModelsIter<'_>;
    fn get_model(&self, index: u32) -> &Model;
}
