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

//! Resolves a texture in a BSP File to an image

use crate::draw::texture::image::LoadableImage;
use stockton_levels::traits::textures::Texture;

use image::{io::Reader, RgbaImage};

use std::path::Path;

/// An object that can be used to resolve a texture from a BSP File
pub trait TextureResolver<T: LoadableImage> {
    /// Get the given texture, or None if it's corrupt/not there.
    fn resolve(&mut self, texture: &Texture) -> Option<T>;
}

/// A basic filesystem resolver which expects no file extension and guesses the image format
pub struct BasicFSResolver<'a> {
    path: &'a Path,
}

impl<'a> BasicFSResolver<'a> {
    pub fn new(path: &'a Path) -> BasicFSResolver<'a> {
        BasicFSResolver { path }
    }
}

impl<'a> TextureResolver<RgbaImage> for BasicFSResolver<'a> {
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
