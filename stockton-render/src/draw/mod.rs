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

//! Given 3D points and some camera information, renders to the screen.

pub mod target;

#[macro_use]
mod macros;
mod buffer;
mod camera;
mod context;
mod draw_buffers;
mod pipeline;
mod render;
mod texture;
mod ui;
mod utils;

pub use self::camera::calc_vp_matrix_system;
pub use self::context::RenderingContext;
pub use self::draw_buffers::UVPoint;
