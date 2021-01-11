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
use crate::draw::texture::{LoadableImage, TextureStore};
use crate::types::*;
use crate::UIState;
use egui::Texture;

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
}

pub fn ensure_textures(
    texture_store: &mut TextureStore,
    ui: &mut UIState,
    device: &mut Device,
    adapter: &mut Adapter,
    allocator: &mut DynamicAllocator,
    command_queue: &mut CommandQueue,
    command_pool: &mut CommandPool,
) {
    let tex = ui.ctx.texture();

    if tex.version != ui.last_tex_ver {
        texture_store
            .put_texture(
                0,
                &*tex,
                device,
                adapter,
                allocator,
                command_queue,
                command_pool,
            )
            .unwrap(); // TODO
        ui.last_tex_ver = tex.version;
    }
}
