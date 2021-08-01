use super::{textures::TextureRef, vertices::Vertex};
use serde::{Deserialize, Serialize};

pub type FaceRef = u32;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub enum Geometry {
    Vertices(Vertex, Vertex, Vertex),
}

pub trait IsFace<C: HasFaces + ?Sized> {
    fn index(&self, container: &C) -> FaceRef;
    fn geometry(&self, container: &C) -> Geometry;
    fn texture_idx(&self, container: &C) -> TextureRef;
}

pub trait HasFaces {
    type Face: IsFace<Self>;

    fn get_face(&self, index: FaceRef) -> Option<&Self::Face>;
    fn iter_faces(&self) -> Faces<Self> {
        Faces {
            next: 0,
            container: self,
        }
    }
}

#[derive(Debug, Clone, Copy)]
pub struct Faces<'a, T: HasFaces + ?Sized> {
    next: FaceRef,
    container: &'a T,
}

impl<'a, T: HasFaces> Iterator for Faces<'a, T> {
    type Item = &'a T::Face;

    fn next(&mut self) -> Option<Self::Item> {
        let res = self.container.get_face(self.next);
        self.next += 1;
        res
    }
}
