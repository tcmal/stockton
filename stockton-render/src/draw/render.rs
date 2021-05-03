use crate::draw::draw_buffers::INITIAL_INDEX_SIZE;
use crate::draw::draw_buffers::INITIAL_VERT_SIZE;
use crate::draw::UvPoint;
use arrayvec::ArrayVec;
use faces::FaceType;
use hal::prelude::*;
use std::convert::TryInto;
use stockton_levels::prelude::*;
use stockton_types::Vector2;

use crate::draw::draw_buffers::DrawBuffers;
use crate::types::*;

use super::texture::TextureRepo;

fn draw_or_queue(
    current_chunk: usize,
    tex_repo: &mut TextureRepo,
    cmd_buffer: &mut CommandBuffer,
    pipeline_layout: &PipelineLayout,
    chunk_start: u32,
    curr_idx_idx: u32,
) {
    if let Some(ds) = tex_repo.attempt_get_descriptor_set(current_chunk) {
        let mut descriptor_sets: ArrayVec<[_; 1]> = ArrayVec::new();
        descriptor_sets.push(ds);
        unsafe {
            cmd_buffer.bind_graphics_descriptor_sets(pipeline_layout, 0, descriptor_sets, &[]);
            cmd_buffer.draw_indexed(chunk_start * 3..(curr_idx_idx * 3) + 1, 0, 0..1);
        }
    } else {
        tex_repo.queue_load(current_chunk).unwrap()
    }
}

pub fn do_render<M: MinBspFeatures<VulkanSystem>>(
    cmd_buffer: &mut CommandBuffer,
    draw_buffers: &mut DrawBuffers<UvPoint>,
    tex_repo: &mut TextureRepo,
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
            // Last index was last of group, so draw it all if textures are loaded.
            draw_or_queue(
                current_chunk,
                tex_repo,
                cmd_buffer,
                pipeline_layout,
                chunk_start as u32,
                curr_idx_idx as u32,
            );

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

                    let uvp = UvPoint(vert.position, face.texture_idx.try_into().unwrap(), uv);
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
    draw_or_queue(
        current_chunk,
        tex_repo,
        cmd_buffer,
        pipeline_layout,
        chunk_start as u32,
        curr_idx_idx as u32,
    );
}
