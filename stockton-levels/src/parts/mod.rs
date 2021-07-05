mod entities;
mod faces;
mod textures;
mod vertices;
mod visdata;

pub mod data {
    pub use super::entities::{Entities, EntityRef};
    pub use super::faces::{FaceRef, Faces, Geometry};
    pub use super::textures::{TextureRef, Textures};
    pub use super::vertices::{Vertex, VertexRef};
}

pub use entities::{HasEntities, IsEntity};
pub use faces::{HasFaces, IsFace};
pub use textures::{HasTextures, IsTexture};
pub use visdata::HasVisData;
