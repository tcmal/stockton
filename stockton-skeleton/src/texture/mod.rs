//! Everything related to loading textures into GPU memory

mod block;
mod image;
mod load;
mod loader;
mod repo;

pub use self::block::TexturesBlock;
pub use self::image::{LoadableImage, TextureResolver};
pub use self::load::TextureLoadConfig;
pub use self::loader::BlockRef;
pub use self::repo::{TexLoadQueue, TextureRepo};

/// The size of each pixel in an image
pub const PIXEL_SIZE: usize = std::mem::size_of::<u8>() * 4;
