//! Given 3D points and some camera information, renders to the screen.

pub mod target;

#[macro_use]
mod macros;
mod buffer;
mod camera;
mod context;
mod depth_buffer;
mod draw_buffers;
mod pipeline;
mod render;
mod texture;
mod ui;
mod utils;

pub use self::camera::calc_vp_matrix_system;
pub use self::context::RenderingContext;
pub use self::draw_buffers::UvPoint;
