//! Common types for all stockton crates.

extern crate stockton_bsp;
extern crate nalgebra as na;

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