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

//! Functions for figuring out what to render
#![allow(dead_code)]

use stockton_levels::prelude::*;
use stockton_levels::traits::tree::{BSPNode, BSPNodeValue};
use stockton_types::Vector3;

/// Get the visible faces according to visdata and frustum culling
// TODO: Write this. For now, just render all faces
pub fn get_visible_faces<X: CoordSystem, T: MinBSPFeatures<X>>(pos: Vector3, file: &T) -> Vec<u32> {
    let vis_cluster = get_cluster_id(pos, file);

    let mut visible = Vec::with_capacity(file.faces_len() as usize);
    if (vis_cluster & 0x80000000) != 0 {
        // Negative = Invalid camera position
        // For now just render everything
        for face_idx in 0..file.faces_len() {
            visible.push(face_idx);
        }

        return visible;
    }

    walk_bsp_tree(file.get_bsp_root(), vis_cluster, &mut visible, file);

    visible
}

pub fn walk_bsp_tree<X: CoordSystem, T: MinBSPFeatures<X>>(
    node: &BSPNode,
    vis_cluster: u32,
    visible_faces: &mut Vec<u32>,
    file: &T,
) {
    if let BSPNodeValue::Children(front, back) = &node.value {
        walk_bsp_tree(back, vis_cluster, visible_faces, file);
        walk_bsp_tree(front, vis_cluster, visible_faces, file);
    } else if let BSPNodeValue::Leaf(leaf) = &node.value {
        if (leaf.cluster_id & 0x80000000) != 0 {
            // Negative means invalid leaf
            return;
        } else if file.cluster_visible_from(vis_cluster, leaf.cluster_id) {
            for face_idx in leaf.faces_idx.iter() {
                // TODO: Culling or something
                visible_faces.push(*face_idx);
            }
        }
    }
}

/// Get the viscluster pos lies in
fn get_cluster_id<X: CoordSystem, T: MinBSPFeatures<X>>(pos: Vector3, file: &T) -> u32 {
    let mut node = file.get_bsp_root();
    while let BSPNodeValue::Children(front, back) = &node.value {
        let plane = file.get_plane(node.plane_idx);
        let dist = plane.normal.dot(&pos) - plane.dist;

        if dist >= 0.0 {
            node = front;
        } else {
            node = back;
        }
    }

    if let BSPNodeValue::Leaf(leaf) = &node.value {
        leaf.cluster_id
    } else {
        panic!("should have had a leaf but didn't");
    }
}
