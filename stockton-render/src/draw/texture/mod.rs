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

//! Everything related to loading textures into GPU memory

mod block;
mod image;
mod load;
mod loader;
mod repo;
pub mod resolver;
mod staging_buffer;

pub use self::block::TexturesBlock;
pub use self::image::LoadableImage;
pub use self::loader::BlockRef;
pub use self::repo::TextureRepo;

/// The size of each pixel in an image
pub const PIXEL_SIZE: usize = std::mem::size_of::<u8>() * 4;
