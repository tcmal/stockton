//! Common types for all stockton crates.

extern crate stockton_bsp;
extern crate nalgebra as na;

use stockton_bsp::BSPFile;

pub mod entity_store;
use entity_store::EntityStore;

/// Alias for convenience
pub type Vector2 = na::base::Vector2<f32>;
/// Alias for convenience
pub type Vector3 = na::base::Vector3<f32>;

/// Alias for convenience
pub type Vector2i = na::base::Vector2<i32>;

/// Alias for convenience
pub type Vector3i = na::base::Vector3<i32>;

/// A live and playable world. There are two parts: The map, which has walls and other static objects,
/// and entities, which can move around and do things and are physics simulated.
pub struct World<'a> {
	pub map: BSPFile<'a>,
	pub live_entities: EntityStore,
}