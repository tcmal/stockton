//! Parsing data from Q3 and similar BSPs

mod brushes;
mod effects;
mod entities;
mod faces;
pub mod file;
mod header;
mod light_maps;
mod light_vols;
mod models;
mod planes;
mod textures;
mod tree;
mod vertices;
mod visdata;

pub use self::file::Q3BspFile;
