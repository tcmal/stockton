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

use crate::draw::texture::TextureRepo;
use arrayvec::ArrayVec;
use hal::prelude::*;
use hal::pso::ShaderStageFlags;

use super::UiPoint;
use crate::draw::draw_buffers::DrawBuffers;
use crate::types::*;
use crate::UiState;
use std::convert::TryInto;
use stockton_types::Vector2;

pub fn do_render(
    cmd_buffer: &mut CommandBuffer,
    pipeline_layout: &PipelineLayout,
    draw_buffers: &mut DrawBuffers<UiPoint>,
    tex_repo: &mut TextureRepo,
    ui: &mut UiState,
) {
    // TODO: Actual UI Rendering
    let (_out, paint) = ui.end_frame();
    let screen = ui.dimensions();

    unsafe {
        cmd_buffer.push_graphics_constants(
            &pipeline_layout,
            ShaderStageFlags::VERTEX,
            0,
            &[screen.x.to_bits(), screen.y.to_bits()],
        );
    }

    for (_rect, tris) in paint.iter() {
        // Copy triangles/indicies
        for i in (0..tris.indices.len()).step_by(3) {
            draw_buffers.index_buffer[i / 3] = (
                tris.indices[i].try_into().unwrap(),
                tris.indices[i + 1].try_into().unwrap(),
                tris.indices[i + 2].try_into().unwrap(),
            );
        }
        for (i, vertex) in tris.vertices.iter().enumerate() {
            draw_buffers.vertex_buffer[i] = UiPoint(
                Vector2::new(vertex.pos.x, vertex.pos.y),
                Vector2::new(vertex.uv.x, vertex.uv.y),
                vertex.color,
            );
        }

        // TODO: *Properly* deal with textures
        if let Some(ds) = tex_repo.attempt_get_descriptor_set(0) {
            let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
            descriptor_sets.push(ds);

            unsafe {
                cmd_buffer.bind_graphics_descriptor_sets(pipeline_layout, 0, descriptor_sets, &[]);
                // Call draw
                cmd_buffer.draw_indexed(0..tris.indices.len() as u32, 0, 0..1);
            }
        } else {
            // tex_repo.queue_load(0);
        }
    }
}
