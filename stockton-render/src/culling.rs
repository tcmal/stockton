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

use stockton_bsp::BSPFile;
use stockton_types::Vector3;

/// Get the visible faces according to visdata and frustum culling
// TODO: Write this. For now, just render all faces
pub fn get_visible_faces<'a>(pos: Vector3, file: &BSPFile) -> Vec<usize> {
	let mut visible = Vec::with_capacity(file.faces.faces.len());
	for x in 0..file.faces.faces.len() {
		visible.push(x);
	}

	return visible;
}

/// Get the viscluster pos lies in 
fn get_cluster_id(pos: Vector3, file: &BSPFile) -> usize {
	let mut node = &file.tree.root;
	while node.leaf.is_none() {
		let plane = file.planes.planes[node.plane_idx as usize];
		let dist = plane.normal.dot(&pos) - plane.dist;

		if dist >= 0.0 {
			node = &node.children.as_ref().unwrap()[0]
		} else {
			node = &node.children.as_ref().unwrap()[1]
		}
	}

	node.leaf.as_ref().unwrap().cluster_id as usize
}