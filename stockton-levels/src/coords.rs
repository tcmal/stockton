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
//! Marker traits for different co-ordinate systems, and functions to swizzle between them

use na::Vector3;
use na::base::Scalar;
use std::ops::Neg;

pub trait CoordSystem {}

/// X points East, Y points South, Z points downwards
pub struct Q3System;
impl CoordSystem for Q3System {}

/// X points east, Y points downwards, Z points inwards
pub struct VulkanSystem;
impl CoordSystem for VulkanSystem {}


pub struct Swizzler;

pub trait SwizzleFromTo<F: CoordSystem, T: CoordSystem> {
	fn swizzle<U: Scalar + Copy + Neg<Output = U>>(vec: &mut Vector3<U>) -> ();
}

impl SwizzleFromTo<Q3System, VulkanSystem> for Swizzler {
	fn swizzle<U: Scalar + Copy + Neg<Output = U>>(vec: &mut Vector3<U>) -> () {
		let temp = 	vec.y;
		vec.y = vec.z;
		vec.z = -temp;
	}
}