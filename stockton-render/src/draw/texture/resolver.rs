// Copyright (C) 2019 Oscar Shrimpton

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Resolves a texture in a BSP File to an image

use stockton_levels::traits::textures::Texture;

use image::{
	RgbaImage,
	io::Reader
};

use std::path::Path;

/// An object that can be used to resolve a texture from a BSP File
pub trait TextureResolver {
	fn resolve(&mut self, texture: &Texture) -> RgbaImage;
}

pub struct BasicFSResolver<'a> {
	path: &'a Path
}

impl<'a> BasicFSResolver<'a> {
	pub fn new(path: &'a Path) -> BasicFSResolver<'a> {
		BasicFSResolver {
			path
		}
	}
}

impl<'a> TextureResolver for BasicFSResolver<'a> {
	fn resolve(&mut self, tex: &Texture) -> RgbaImage {
		let path = self.path.join(&tex.name);
		println!("Loading texture from {:?}", path);

		Reader::open(path).unwrap()
			.with_guessed_format().unwrap()
			.decode().unwrap()
			.into_rgba()
	}
}