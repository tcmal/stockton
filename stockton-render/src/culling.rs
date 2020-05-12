// Copyright (C) 2019 Oscar Shrimpton

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

//! Functions for figuring out what to render
#![allow(dead_code)]

use stockton_levels::prelude::*;
use stockton_levels::traits::tree::BSPNodeValue;
use stockton_types::Vector3;


/// Get the visible faces according to visdata and frustum culling
// TODO: Write this. For now, just render all faces
pub fn get_visible_faces<T: MinBSPFeatures>(_pos: Vector3, file: &T) -> Vec<u32> {
	let mut visible = Vec::with_capacity(file.faces_len() as usize);
	for x in 0..file.faces_len() {
		visible.push(x as u32);
	}

	return visible;
}

/// Get the viscluster pos lies in 
fn get_cluster_id<T: MinBSPFeatures>(pos: Vector3, file: &T) -> u32 {
	let mut node = file.get_bsp_root();
	loop {
		if let BSPNodeValue::Children(front, back) = &node.value{
			let plane = file.get_plane(node.plane_idx);
			let dist = plane.normal.dot(&pos) - plane.dist;

			if dist >= 0.0 {
				node = front;
			} else {
				node = back;
			}
		} else {
			break;
		}
	}

	if let BSPNodeValue::Leaf(leaf) = &node.value {
		leaf.cluster_id
	} else {
		panic!("should have had a leaf but didn't");
	}
}