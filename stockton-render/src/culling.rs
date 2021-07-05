//! Functions for figuring out what to render
#![allow(dead_code)]

use stockton_levels::prelude::*;
use stockton_levels::parts::tree::{BspNode, BspNodeValue};
use stockton_types::Vector3;

/// Get the visible faces according to visdata and frustum culling
// TODO: Write this. For now, just render all faces
pub fn get_visible_faces<X: CoordSystem, T: MinBspFeatures<X>>(pos: Vector3, file: &T) -> Vec<u32> {
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

pub fn walk_bsp_tree<X: CoordSystem, T: MinBspFeatures<X>>(
    node: &BspNode,
    vis_cluster: u32,
    visible_faces: &mut Vec<u32>,
    file: &T,
) {
    if let BspNodeValue::Children(front, back) = &node.value {
        walk_bsp_tree(back, vis_cluster, visible_faces, file);
        walk_bsp_tree(front, vis_cluster, visible_faces, file);
    } else if let BspNodeValue::Leaf(leaf) = &node.value {
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
fn get_cluster_id<X: CoordSystem, T: MinBspFeatures<X>>(pos: Vector3, file: &T) -> u32 {
    let mut node = file.get_bsp_root();
    while let BspNodeValue::Children(front, back) = &node.value {
        let plane = file.get_plane(node.plane_idx);
        let dist = plane.normal.dot(&pos) - plane.dist;

        if dist >= 0.0 {
            node = front;
        } else {
            node = back;
        }
    }

    if let BspNodeValue::Leaf(leaf) = &node.value {
        leaf.cluster_id
    } else {
        panic!("should have had a leaf but didn't");
    }
}
