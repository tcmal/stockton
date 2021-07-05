use crate::draw::texture::TextureRepo;
use hal::pso::{Rect, ShaderStageFlags};

use super::UiPoint;
use crate::draw::draw_buffers::DrawBuffers;
use crate::types::*;
use crate::UiState;
use anyhow::{anyhow, Result};
use egui::{ClippedMesh, TextureId};
use std::{array::IntoIter, convert::TryInto, iter::empty};
use stockton_types::Vector2;

pub fn do_render(
    cmd_buffer: &mut CommandBufferT,
    pipeline_layout: &PipelineLayoutT,
    draw_buffers: &mut DrawBuffers<UiPoint>,
    tex_repo: &mut TextureRepo,
    ui: &mut UiState,
) -> Result<()> {
    // TODO: Actual UI Rendering
    let (_out, shapes) = ui.end_frame();
    let screen = ui.dimensions().ok_or(anyhow!("UI not set up properly."))?;
    let shapes = ui.ctx().tessellate(shapes);

    for ClippedMesh(rect, tris) in shapes.iter() {
        assert!(tris.texture_id == TextureId::Egui);

        // Copy triangles/indicies
        for i in (0..tris.indices.len()).step_by(3) {
            draw_buffers.index_buffer[i / 3] = (
                tris.indices[i].try_into()?,
                tris.indices[i + 1].try_into()?,
                tris.indices[i + 2].try_into()?,
            );
            // eprintln!(
            //     "{} {}",
            //     tris.vertices[tris.indices[i] as usize].uv.x,
            //     tris.vertices[tris.indices[i] as usize].uv.y
            // );
            // eprintln!(
            //     "{} {}",
            //     tris.vertices[tris.indices[i + 1] as usize].uv.x,
            //     tris.vertices[tris.indices[i + 1] as usize].uv.y
            // );
            // eprintln!(
            //     "{} {}",
            //     tris.vertices[tris.indices[i + 2] as usize].uv.x,
            //     tris.vertices[tris.indices[i + 2] as usize].uv.y
            // );
        }
        for (i, vertex) in tris.vertices.iter().enumerate() {
            draw_buffers.vertex_buffer[i] = UiPoint(
                Vector2::new(vertex.pos.x, vertex.pos.y),
                Vector2::new(vertex.uv.x, vertex.uv.y),
                [
                    vertex.color.r() as f32 / 255.0,
                    vertex.color.g() as f32 / 255.0,
                    vertex.color.b() as f32 / 255.0,
                    vertex.color.a() as f32 / 255.0,
                ],
            );
        }
        // TODO: *Properly* deal with textures
        if let Some(ds) = tex_repo.attempt_get_descriptor_set(0) {
            unsafe {
                cmd_buffer.push_graphics_constants(
                    pipeline_layout,
                    ShaderStageFlags::VERTEX,
                    0,
                    &[screen.x.to_bits(), screen.y.to_bits()],
                );

                cmd_buffer.set_scissors(
                    0,
                    IntoIter::new([Rect {
                        x: rect.min.x as i16,
                        y: rect.min.y as i16,
                        w: rect.width() as i16,
                        h: rect.height() as i16,
                    }]),
                );
                cmd_buffer.bind_graphics_descriptor_sets(
                    pipeline_layout,
                    0,
                    IntoIter::new([ds]),
                    empty(),
                );
                // Call draw
                cmd_buffer.draw_indexed(0..tris.indices.len() as u32, 0, 0..1);
            }
        } else {
            // tex_repo.queue_load(0);
        }
    }

    Ok(())
}
