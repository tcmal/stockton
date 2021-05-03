//! Helper functions for parsing

use na::{Vector2, Vector3};
use std::convert::TryInto;

/// Turn a slice into a le i32, the int datatype in a bsp file.
/// # Panics
/// If slice is not 4 bytes long
pub fn slice_to_i32(slice: &[u8]) -> i32 {
    i32::from_le_bytes(slice.try_into().unwrap())
}

/// Turn a slice into a le u32, used for some bitflags.
/// # Panics
/// If slice is not 4 bytes long.
pub fn slice_to_u32(slice: &[u8]) -> u32 {
    u32::from_le_bytes(slice.try_into().unwrap())
}

/// Turn a slice into a le f32, the float datatype in a bsp file.
/// # Panics
/// If slice is not 4 bytes long
pub fn slice_to_f32(slice: &[u8]) -> f32 {
    f32::from_bits(u32::from_le_bytes(slice.try_into().unwrap()))
}

/// Turn a slice of floats into a 3D vector
/// # Panics
/// If slice isn't 12 bytes long.
pub fn slice_to_vec3(slice: &[u8]) -> Vector3<f32> {
    Vector3::new(
        slice_to_f32(&slice[0..4]),
        slice_to_f32(&slice[4..8]),
        slice_to_f32(&slice[8..12]),
    )
}

/// Turn a slice of i32s into a 3D vector
/// # Panics
/// If slice isn't 12 bytes long.
pub fn slice_to_vec3i(slice: &[u8]) -> Vector3<i32> {
    Vector3::new(
        slice_to_i32(&slice[0..4]),
        slice_to_i32(&slice[4..8]),
        slice_to_i32(&slice[8..12]),
    )
}

/// Turn a slice of u32s into a 2D vector
/// # Panics
/// If slice isn't 8 bytes long.
pub fn slice_to_vec2ui(slice: &[u8]) -> Vector2<u32> {
    Vector2::new(slice_to_u32(&slice[0..4]), slice_to_u32(&slice[4..8]))
}
