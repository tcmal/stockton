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

use crate::draw::draw_buffers::INITIAL_INDEX_SIZE;
use crate::draw::draw_buffers::INITIAL_VERT_SIZE;
use crate::draw::texture::TextureStore;
use crate::draw::UVPoint;
use arrayvec::ArrayVec;
use faces::FaceType;
use hal::prelude::*;
use std::convert::TryInto;
use stockton_levels::prelude::*;
use stockton_types::Vector2;

use crate::draw::draw_buffers::DrawBuffers;
use crate::types::*;

pub fn do_render<M: MinBSPFeatures<VulkanSystem>>(
    cmd_buffer: &mut CommandBuffer,
    draw_buffers: &mut DrawBuffers,
    texture_store: &TextureStore,
    pipeline_layout: &PipelineLayout,
    file: &M,
    faces: &[u32],
) {
    // Iterate over faces, copying them in and drawing groups that use the same texture chunk all at once.
    let mut current_chunk = file.get_face(0).texture_idx as usize / 8;
    let mut chunk_start = 0;

    let mut curr_vert_idx: usize = 0;
    let mut curr_idx_idx: usize = 0;

    for face in faces.iter().map(|idx| file.get_face(*idx)) {
        if current_chunk != face.texture_idx as usize / 8 {
            // Last index was last of group, so draw it all.
            let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
            descriptor_sets.push(texture_store.get_chunk_descriptor_set(current_chunk));
            unsafe {
                cmd_buffer.bind_graphics_descriptor_sets(pipeline_layout, 0, descriptor_sets, &[]);
                cmd_buffer.draw_indexed(
                    chunk_start as u32 * 3..(curr_idx_idx as u32 * 3) + 1,
                    0,
                    0..1,
                );
            }

            // Next group of same-chunked faces starts here.
            chunk_start = curr_idx_idx;
            current_chunk = face.texture_idx as usize / 8;
        }

        if face.face_type == FaceType::Polygon || face.face_type == FaceType::Mesh {
            // 2 layers of indirection
            let base = face.vertices_idx.start;

            for idx in face.meshverts_idx.clone().step_by(3) {
                let start_idx: u16 = curr_vert_idx.try_into().unwrap();

                for idx2 in idx..idx + 3 {
                    let vert = &file.resolve_meshvert(idx2 as u32, base);
                    let uv = Vector2::new(vert.tex.u[0], vert.tex.v[0]);

                    let uvp = UVPoint(vert.position, face.texture_idx.try_into().unwrap(), uv);
                    draw_buffers.vertex_buffer[curr_vert_idx] = uvp;

                    curr_vert_idx += 1;
                }

                draw_buffers.index_buffer[curr_idx_idx] = (start_idx, start_idx + 1, start_idx + 2);

                curr_idx_idx += 1;

                if curr_vert_idx >= INITIAL_VERT_SIZE.try_into().unwrap()
                    || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into().unwrap()
                {
                    println!("out of vertex buffer space!");
                    break;
                }
            }
        } else {
            // TODO: Other types of faces
        }

        if curr_vert_idx >= INITIAL_VERT_SIZE.try_into().unwrap()
            || curr_idx_idx >= INITIAL_INDEX_SIZE.try_into().unwrap()
        {
            println!("out of vertex buffer space!");
            break;
        }
    }

    // Draw the final group of chunks
    let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
    descriptor_sets.push(texture_store.get_chunk_descriptor_set(current_chunk));
    unsafe {
        cmd_buffer.bind_graphics_descriptor_sets(&pipeline_layout, 0, descriptor_sets, &[]);
        cmd_buffer.draw_indexed(
            chunk_start as u32 * 3..(curr_idx_idx as u32 * 3) + 1,
            0,
            0..1,
        );
    }
}
