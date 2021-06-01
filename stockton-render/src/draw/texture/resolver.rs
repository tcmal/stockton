//! Resolves a texture in a BSP File to an image

use crate::draw::texture::image::LoadableImage;
use stockton_levels::prelude::HasTextures;

use std::{
    mem::drop,
    path::Path,
    sync::{Arc, RwLock},
};

use image::{io::Reader, RgbaImage};

/// An object that can be used to resolve a texture from a BSP File
pub trait TextureResolver {
    type Image: LoadableImage;

    /// Get the given texture, or None if it's corrupt/not there.
    fn resolve(&mut self, texture_id: u32) -> Option<Self::Image>;
}

/// A basic filesystem resolver which gets the texture name from any HasTextures Object.
pub struct FsResolver<'a, T: HasTextures> {
    path: &'a Path,
    map_lock: Arc<RwLock<T>>,
}

impl<'a, T: HasTextures> FsResolver<'a, T> {
    pub fn new(path: &'a Path, map_lock: Arc<RwLock<T>>) -> Self {
        FsResolver { path, map_lock }
    }
}

impl<'a, T: HasTextures> TextureResolver for FsResolver<'a, T> {
    type Image = RgbaImage;

    fn resolve(&mut self, tex: u32) -> Option<Self::Image> {
        let map = self.map_lock.read().unwrap();
        let tex = map.get_texture(tex)?;
        let path = self.path.join(&tex.name);

        drop(tex);
        drop(map);

        if let Ok(file) = Reader::open(path) {
            if let Ok(guessed) = file.with_guessed_format() {
                if let Ok(decoded) = guessed.decode() {
                    return Some(decoded.into_rgba8());
                }
            }
        }

        None
    }
}
