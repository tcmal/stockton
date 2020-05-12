// Copyright (C) 2019 Oscar Shrimpton
//
// This file is part of rust_bsp.
//
// rust_bsp is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust_bsp is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust_bsp.  If not, see <http://www.gnu.org/licenses/>.

//! Parses the BSP tree into a usable format

use crate::helpers::{slice_to_u32, slice_to_i32, slice_to_vec3i};
use crate::types::{ParseError, Result};
use crate::traits::tree::*;
use super::Q3BSPFile;

const NODE_SIZE: usize = 4 + (4 * 2) + (4 * 3) + (4 * 3);
const LEAF_SIZE: usize = 4 * 6 + (4 * 3 * 2);

pub fn from_data(
    nodes: &[u8],
    leaves: &[u8],
    leaf_faces: &[u8],
    leaf_brushes: &[u8],
    n_faces: u32,
    n_brushes: u32,
) -> Result<BSPNode> {
    if nodes.len() % NODE_SIZE != 0 || leaves.len() % LEAF_SIZE != 0 {
        return Err(ParseError::Invalid);
    }

    Ok(compile_node(
            0,
            nodes,
            leaves,
            leaf_faces,
            leaf_brushes,
            n_faces,
            n_brushes,
        )?,
    )
}

    /// Internal function. Visits given node and all its children. Used to recursively build tree.
fn compile_node(
    i: i32,
    nodes: &[u8],
    leaves: &[u8],
    leaf_faces: &[u8],
    leaf_brushes: &[u8],
    n_faces: u32,
    n_brushes: u32,
) -> Result<BSPNode> {
    if i < 0 {
        // Leaf.
        let i = i.abs() - 1;

        let raw = &leaves[i as usize * LEAF_SIZE..(i as usize * LEAF_SIZE) + LEAF_SIZE];

        let faces_idx = {
            let start = slice_to_u32(&raw[32..36]) as usize;
            let n = slice_to_u32(&raw[36..40]) as usize;

            let mut faces = Vec::with_capacity(n);
            if n > 0 {
                if start + n > leaf_faces.len() / 4 {
                    return Err(ParseError::Invalid);
                }

                for i in start..start + n {
                    let face_idx = slice_to_u32(&leaf_faces[i * 4..(i + 1) * 4]);
                    if face_idx >= n_faces {
                        return Err(ParseError::Invalid);
                    }

                    faces.push(face_idx);
                }
            }

            faces.into_boxed_slice()
        };

        let brushes_idx = {
            let start = slice_to_u32(&raw[40..44]) as usize;
            let n = slice_to_u32(&raw[44..48]) as usize;
            let mut brushes = Vec::with_capacity(n);
        
            if n > 0 {
                if start + n > leaf_brushes.len() / 4 {
                    return Err(ParseError::Invalid);
                }

                for i in start..start + n {
                    let brush_idx = slice_to_u32(&leaf_brushes[i * 4..(i + 1) * 4]);
                    if brush_idx >= n_brushes {
                        return Err(ParseError::Invalid);
                    }

                    brushes.push(brush_idx);
                }
            }

            brushes.into_boxed_slice()
        };

        let leaf = BSPLeaf {
            cluster_id: slice_to_u32(&raw[0..4]),
            area: slice_to_i32(&raw[4..8]),
            // 8..20 = min
            // 20..32 = max
            faces_idx,
            brushes_idx,
        };

        Ok(BSPNode {
            plane_idx: 0,
            min: slice_to_vec3i(&raw[8..20]),
            max: slice_to_vec3i(&raw[20..32]),
            value: BSPNodeValue::Leaf(leaf),
        })
    } else {
        // Node.
        let raw = &nodes[i as usize * NODE_SIZE..(i as usize * NODE_SIZE) + NODE_SIZE];

        let plane_idx = slice_to_u32(&raw[0..4]);
        let child_one = compile_node(
            slice_to_i32(&raw[4..8]),
            nodes,
            leaves,
            leaf_faces,
            leaf_brushes,
            n_faces,
            n_brushes,
        )?;
        let child_two = compile_node(
            slice_to_i32(&raw[8..12]),
            nodes,
            leaves,
            leaf_faces,
            leaf_brushes,
            n_faces,
            n_brushes,
        )?;
        let min = slice_to_vec3i(&raw[12..24]);
        let max = slice_to_vec3i(&raw[24..36]);

        Ok(BSPNode {
            plane_idx,
            value: BSPNodeValue::Children(Box::new(child_one), Box::new(child_two)),
            min,
            max,
        })
    }
}

impl HasBSPTree for Q3BSPFile {
    fn get_bsp_root<'a>(&'a self) -> &'a BSPNode {
        &self.tree_root
    }
}