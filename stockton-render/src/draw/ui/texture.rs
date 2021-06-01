use crate::draw::texture::{resolver::TextureResolver, LoadableImage, TextureRepo};
use crate::UiState;
use anyhow::Result;
use egui::{CtxRef, Texture};
use log::debug;
use std::{convert::TryInto, sync::Arc};

pub struct UiTextures {
    ctx: CtxRef,
}

impl TextureResolver for UiTextures {
    type Image = Arc<Texture>;
    fn resolve(&mut self, tex: u32) -> Option<Self::Image> {
        if tex == 0 {
            Some(self.ctx.texture())
        } else {
            None
        }
    }
}

impl UiTextures {
    pub fn new(ctx: CtxRef) -> Self {
        UiTextures { ctx }
    }
}

impl LoadableImage for Arc<Texture> {
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
                *ptr.offset(i as isize * 4) = 255;
                *ptr.offset((i as isize * 4) + 1) = 255;
                *ptr.offset((i as isize * 4) + 2) = 255;
                *ptr.offset((i as isize * 4) + 3) = *x;
            }
        }
    }

    unsafe fn copy_into(&self, ptr: *mut u8, row_size: usize) {
        for y in 0..self.height() {
            self.copy_row(y, ptr.offset((row_size * y as usize).try_into().unwrap()));
        }
    }
}

pub fn ensure_textures(tex_repo: &mut TextureRepo, ui: &mut UiState) -> Result<()> {
    let tex = ui.ctx().texture();

    if tex.version != ui.last_tex_ver {
        debug!("Queueing UI Texture reload");
        tex_repo.force_queue_load(0)?;
        ui.last_tex_ver = tex.version;
    }

    Ok(())
}
