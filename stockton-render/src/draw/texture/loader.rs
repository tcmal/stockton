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

//! Deals with loading textures into GPU memory

use super::chunk::TextureChunk;
use crate::draw::texture::chunk::CHUNK_SIZE;
use crate::draw::texture::resolver::BasicFSResolver;
use core::mem::ManuallyDrop;
use std::path::Path;

use log::debug;

use hal::prelude::*;

use stockton_levels::prelude::*;

use crate::error;
use crate::types::*;

/// Stores all loaded textures in GPU memory.
/// When rendering, the descriptor sets are bound to the buffer
/// The descriptor set layout should have the same count of textures as this does.
/// All descriptors will be properly initialised images.
pub struct TextureStore {
    descriptor_pool: ManuallyDrop<DescriptorPool>,
    pub(crate) descriptor_set_layout: ManuallyDrop<DescriptorSetLayout>,
    chunks: Box<[TextureChunk]>,
}

impl TextureStore {
    /// Create a new texture store for the given file, loading all textures from it.
    pub fn new<T: HasTextures>(
        device: &mut Device,
        adapter: &mut Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
        file: &T,
    ) -> Result<TextureStore, error::CreationError> {
        // Figure out how many textures in this file / how many chunks needed
        let size = file.textures_iter().count();
        let num_chunks = {
            let mut x = size / CHUNK_SIZE;
            if size % CHUNK_SIZE != 0 {
                x += 1;
            }
            x
        };
        let rounded_size = num_chunks * CHUNK_SIZE;

        // Descriptor pool, where we get our sets from
        let mut descriptor_pool = unsafe {
            use hal::pso::{
                DescriptorPoolCreateFlags, DescriptorRangeDesc, DescriptorType, ImageDescriptorType,
            };

            device
                .create_descriptor_pool(
                    num_chunks,
                    &[
                        DescriptorRangeDesc {
                            ty: DescriptorType::Image {
                                ty: ImageDescriptorType::Sampled {
                                    with_sampler: false,
                                },
                            },
                            count: rounded_size,
                        },
                        DescriptorRangeDesc {
                            ty: DescriptorType::Sampler,
                            count: rounded_size,
                        },
                    ],
                    DescriptorPoolCreateFlags::empty(),
                )
                .map_err(|e| {
                    println!("{:?}", e);
                    error::CreationError::OutOfMemoryError
                })?
        };

        // Layout of our descriptor sets
        let descriptor_set_layout = unsafe {
            use hal::pso::{
                DescriptorSetLayoutBinding, DescriptorType, ImageDescriptorType, ShaderStageFlags,
            };

            device.create_descriptor_set_layout(
                &[
                    DescriptorSetLayoutBinding {
                        binding: 0,
                        ty: DescriptorType::Image {
                            ty: ImageDescriptorType::Sampled {
                                with_sampler: false,
                            },
                        },
                        count: CHUNK_SIZE,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                    DescriptorSetLayoutBinding {
                        binding: 1,
                        ty: DescriptorType::Sampler,
                        count: CHUNK_SIZE,
                        stage_flags: ShaderStageFlags::FRAGMENT,
                        immutable_samplers: false,
                    },
                ],
                &[],
            )
        }
        .map_err(|_| error::CreationError::OutOfMemoryError)?;

        // TODO: Proper way to set up resolver
        let mut resolver = BasicFSResolver::new(Path::new("."));

        // Create texture chunks
        debug!("Starting to load textures...");
        let mut chunks = Vec::with_capacity(num_chunks);
        for i in 0..num_chunks {
            debug!("Chunk {} / {}", i + 1, num_chunks);

            let descriptor_set = unsafe {
                descriptor_pool
                    .allocate_set(&descriptor_set_layout)
                    .map_err(|_| error::CreationError::OutOfMemoryError)?
            };
            chunks.push(TextureChunk::new(
                device,
                adapter,
                command_queue,
                command_pool,
                descriptor_set,
                file.textures_iter().skip(i * CHUNK_SIZE).take(CHUNK_SIZE),
                &mut resolver,
            )?);
        }

        debug!("All textures loaded.");

        Ok(TextureStore {
            descriptor_pool: ManuallyDrop::new(descriptor_pool),
            descriptor_set_layout: ManuallyDrop::new(descriptor_set_layout),
            chunks: chunks.into_boxed_slice(),
        })
    }

    /// Call this before dropping
    pub fn deactivate(mut self, device: &mut Device) {
        unsafe {
            use core::ptr::read;

            for chunk in self.chunks.into_vec().drain(..) {
                chunk.deactivate(device)
            }

            self.descriptor_pool.reset();
            device.destroy_descriptor_set_layout(ManuallyDrop::into_inner(read(
                &self.descriptor_set_layout,
            )));
            device.destroy_descriptor_pool(ManuallyDrop::into_inner(read(&self.descriptor_pool)));
        }
    }

    /// Get the descriptor set for a given chunk
    pub fn get_chunk_descriptor_set(&self, idx: usize) -> &DescriptorSet {
        &self.chunks[idx].descriptor_set
    }
}
