// Copyright (C) Oscar Shrimpton 2019  

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.
//! Common types for all stockton crates.

extern crate stockton_bsp;
extern crate nalgebra_glm as na;
#[macro_use]
extern crate downcast_rs;

pub mod entity_store;
pub use entity_store::{EntityStore, Entity};

pub mod world;
pub use world::World;

pub mod ent_map;

/// Alias for convenience
pub type Vector2 = na::Vec2;
/// Alias for convenience
pub type Vector3 = na::Vec3;

/// Alias for convenience
pub type Vector2i = na::IVec2;

/// Alias for convenience
pub type Vector3i = na::IVec3;

/// Alias for convenience
pub type Matrix4 = na::Mat4x4;