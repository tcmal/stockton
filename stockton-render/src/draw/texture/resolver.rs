//! Resolves a texture in a BSP File to an image

use crate::draw::texture::image::LoadableImage;
use stockton_levels::traits::textures::Texture;

use std::path::Path;

use image::{io::Reader, RgbaImage};

/// An object that can be used to resolve a texture from a BSP File
pub trait TextureResolver<T: LoadableImage> {
    /// Get the given texture, or None if it's corrupt/not there.
    fn resolve(&mut self, texture: &Texture) -> Option<T>;
}

/// A basic filesystem resolver which expects no file extension and guesses the image format
pub struct BasicFsResolver<'a> {
    path: &'a Path,
}

impl<'a> BasicFsResolver<'a> {
    pub fn new(path: &'a Path) -> BasicFsResolver<'a> {
        BasicFsResolver { path }
    }
}

impl<'a> TextureResolver<RgbaImage> for BasicFsResolver<'a> {
    fn resolve(&mut self, tex: &Texture) -> Option<RgbaImage> {
        let path = self.path.join(&tex.name);

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
