//! Common types for all stockton crates.

extern crate nalgebra_glm as na;

pub mod components;
pub mod session;

pub use session::Session;

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
