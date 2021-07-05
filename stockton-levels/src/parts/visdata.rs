use super::faces::FaceRef;
use na::Vector3;
use std::iter::Iterator;

pub trait HasVisData {
    type Faces: Iterator<Item = FaceRef>;
    fn get_visible(pos: Vector3<f32>, rot: Vector3<f32>) -> Self::Faces;
}
