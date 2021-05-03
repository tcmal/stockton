use crate::draw::texture::{LoadableImage, TextureRepo};
use crate::types::*;
use crate::UiState;
use egui::Texture;
use stockton_levels::{prelude::HasTextures, traits::textures::Texture as LTexture};

pub struct UiTextures;

impl HasTextures for UiTextures {
    type TexturesIter<'a> = std::slice::Iter<'a, LTexture>;

    fn textures_iter(&self) -> Self::TexturesIter<'_> {
        (&[]).iter()
    }

    fn get_texture(&self, _idx: u32) -> Option<&stockton_levels::prelude::textures::Texture> {
        None
    }
}

impl LoadableImage for &Texture {
    fn width(&self) -> u32 {
        self.width as u32
    }
    fn height(&self) -> u32 {
        self.height as u32
    }
    fn copy_row(&self, y: u32, ptr: *mut u8) {
        let row_size = self.width();
        let pixels = &self.pixels[(y * row_size) as usize..((y + 1) * row_size) as usize];

        for (i, x) in pixels.iter().enumerate() {
            unsafe {
                *ptr.offset(i as isize * 3) = *x;
                *ptr.offset((i as isize * 3) + 1) = *x;
                *ptr.offset((i as isize * 3) + 2) = *x;
            }
        }
    }

    unsafe fn copy_into(&self, _ptr: *mut u8, _row_size: usize) {
        todo!()
    }
}

pub fn ensure_textures(
    _tex_repo: &mut TextureRepo,
    ui: &mut UiState,
    _device: &mut Device,
    _adapter: &mut Adapter,
    _allocator: &mut DynamicAllocator,
    _command_queue: &mut CommandQueue,
    _command_pool: &mut CommandPool,
) {
    let tex = ui.ctx.texture();

    if tex.version != ui.last_tex_ver {
        // tex_repo.force_queue_load(0).unwrap(); // TODO
        ui.last_tex_ver = tex.version;
    }
}
