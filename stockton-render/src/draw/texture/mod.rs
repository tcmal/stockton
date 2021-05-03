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
