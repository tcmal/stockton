use super::faces::FaceRef;
use std::iter::Iterator;
use stockton_types::components::{CameraSettings, Transform};

pub trait HasVisData<'a> {
    type Faces: Iterator<Item = FaceRef>;
    fn get_visible(&'a self, transform: &Transform, settings: &CameraSettings) -> Self::Faces;
}
