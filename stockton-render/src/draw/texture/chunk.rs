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

//! A chunk of textures is an array of textures, the size of which is known at compile time.
//! This reduces the number of times we need to re-bind our descriptor sets

use crate::draw::texture::image::LoadableImage;
use hal::prelude::*;
use image::{Rgba, RgbaImage};

use core::mem::replace;
use std::ops::Deref;

use crate::{error, types::*};

use super::image::SampledImage;
use super::resolver::TextureResolver;
use log::debug;
use std::iter::Iterator;
use stockton_levels::traits::textures::Texture;

/// The size of a chunk. Needs to match up with the fragment shader
pub const CHUNK_SIZE: usize = 8;

/// An array of textures
pub struct TextureChunk {
    pub(crate) descriptor_set: DescriptorSet,
    sampled_images: Vec<SampledImage>,
}

impl TextureChunk {
    /// Create a new empty texture chunk
    pub fn new_empty(
        device: &mut Device,
        adapter: &mut Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
        descriptor_set: DescriptorSet,
    ) -> Result<TextureChunk, error::CreationError> {
        let mut store = TextureChunk {
            descriptor_set,
            sampled_images: Vec::with_capacity(CHUNK_SIZE),
        };

        for i in 0..CHUNK_SIZE {
            debug!("Putting a placeholder in slot {}", i);
            store
                .put_texture(
                    RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 1])),
                    i,
                    device,
                    adapter,
                    command_queue,
                    command_pool,
                )
                .unwrap();
        }

        Ok(store)
    }

    /// Create a new texture chunk and load in the textures specified by `range` from `file` using `resolver`
    /// Can error if the descriptor pool is too small or if a texture isn't found
    pub fn new<'a, I, R: TextureResolver<T>, T: LoadableImage>(
        device: &mut Device,
        adapter: &mut Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
        descriptor_set: DescriptorSet,
        textures: I,
        resolver: &mut R,
    ) -> Result<TextureChunk, error::CreationError>
    where
        I: 'a + Iterator<Item = &'a Texture>,
    {
        let mut store = TextureChunk {
            descriptor_set,
            sampled_images: Vec::with_capacity(CHUNK_SIZE),
        };

        let mut local_idx = 0;

        debug!("Created descriptor set");
        for tex in textures {
            if let Some(img) = resolver.resolve(tex) {
                store
                    .put_texture(img, local_idx, device, adapter, command_queue, command_pool)
                    .unwrap();
            } else {
                // Texture not found. For now, tear everything down.
                store.deactivate(device);

                return Err(error::CreationError::BadDataError);
            }

            local_idx += 1;
        }

        // Pad out the end if needed
        while local_idx < CHUNK_SIZE {
            debug!("Putting a placeholder in slot {}", local_idx);
            store
                .put_texture(
                    RgbaImage::from_pixel(1, 1, Rgba([0, 0, 0, 1])),
                    local_idx,
                    device,
                    adapter,
                    command_queue,
                    command_pool,
                )
                .unwrap();

            local_idx += 1;
        }

        Ok(store)
    }

    pub fn put_texture<T: LoadableImage>(
        &mut self,
        image: T,
        idx: usize,
        device: &mut Device,
        adapter: &mut Adapter,
        command_queue: &mut CommandQueue,
        command_pool: &mut CommandPool,
    ) -> Result<(), &'static str> {
        // Load the image
        let texture = SampledImage::load_into_new(
            image,
            device,
            adapter,
            command_queue,
            command_pool,
            hal::format::Format::Rgba8Srgb, // TODO
            hal::image::Usage::empty(),
        )?;

        // Write it to the descriptor set
        unsafe {
            use hal::image::Layout;
            use hal::pso::{Descriptor, DescriptorSetWrite};

            device.write_descriptor_sets(vec![
                DescriptorSetWrite {
                    set: &self.descriptor_set,
                    binding: 0,
                    array_offset: idx,
                    descriptors: Some(Descriptor::Image(
                        texture.image.image_view.deref(),
                        Layout::ShaderReadOnlyOptimal,
                    )),
                },
                DescriptorSetWrite {
                    set: &self.descriptor_set,
                    binding: 1,
                    array_offset: idx,
                    descriptors: Some(Descriptor::Sampler(texture.sampler.deref())),
                },
            ]);
        };

        // Store it so we can safely deactivate it when we need to
        // Deactivate the old image if we need to
        if idx < self.sampled_images.len() {
            replace(&mut self.sampled_images[idx], texture).deactivate(device);
        } else {
            self.sampled_images.push(texture);
        }

        Ok(())
    }

    pub fn deactivate(mut self, device: &mut Device) {
        for img in self.sampled_images.drain(..) {
            img.deactivate(device);
        }
    }
}
