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

//! Convenience module to reference types that are stored in the backend's enum

pub type Device = <back::Backend as hal::Backend>::Device;
pub type Buffer = <back::Backend as hal::Backend>::Buffer;
pub type Memory = <back::Backend as hal::Backend>::Memory;
pub type Swapchain = <back::Backend as hal::Backend>::Swapchain;
pub type Surface = <back::Backend as hal::Backend>::Surface;
pub type Semaphore = <back::Backend as hal::Backend>::Semaphore;
pub type Fence = <back::Backend as hal::Backend>::Fence;
pub type CommandPool = <back::Backend as hal::Backend>::CommandPool;
pub type CommandBuffer = <back::Backend as hal::Backend>::CommandBuffer;
pub type CommandQueue = <back::Backend as hal::Backend>::CommandQueue;
pub type DescriptorSetLayout = <back::Backend as hal::Backend>::DescriptorSetLayout;
pub type DescriptorPool = <back::Backend as hal::Backend>::DescriptorPool;
pub type DescriptorSet = <back::Backend as hal::Backend>::DescriptorSet;
pub type PipelineLayout = <back::Backend as hal::Backend>::PipelineLayout;
pub type GraphicsPipeline = <back::Backend as hal::Backend>::GraphicsPipeline;
pub type ShaderModule = <back::Backend as hal::Backend>::ShaderModule;
pub type Sampler = <back::Backend as hal::Backend>::Sampler;
pub type Image = <back::Backend as hal::Backend>::Image;
pub type ImageView = <back::Backend as hal::Backend>::ImageView;
pub type Framebuffer = <back::Backend as hal::Backend>::Framebuffer;
pub type RenderPass = <back::Backend as hal::Backend>::RenderPass;

pub type Adapter = hal::adapter::Adapter<back::Backend>;
pub type QueueGroup = hal::queue::QueueGroup<back::Backend>;

pub type DynamicAllocator = rendy_memory::DynamicAllocator<back::Backend>;
pub type DynamicBlock = rendy_memory::DynamicBlock<back::Backend>;
