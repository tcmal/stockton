use egui::paint::color::Srgba;

pub mod pipeline;
pub mod render;
pub mod texture;

pub use pipeline::UiPipeline;
pub use render::do_render;
use stockton_types::Vector2;
pub use texture::{ensure_textures, UiTextures};

#[derive(Debug)]
pub struct UiPoint(pub Vector2, pub Vector2, pub Srgba);
