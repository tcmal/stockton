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
extern crate nalgebra as na;
#[macro_use]
extern crate downcast_rs;

pub mod entity_store;
pub use entity_store::{EntityStore, Entity};

pub mod world;
pub use world::World;

/// Alias for convenience
pub type Vector2 = na::base::Vector2<f32>;
/// Alias for convenience
pub type Vector3 = na::base::Vector3<f32>;

/// Alias for convenience
pub type Vector2i = na::base::Vector2<i32>;

/// Alias for convenience
pub type Vector3i = na::base::Vector3<i32>;