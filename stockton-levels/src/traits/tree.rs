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

use na::Vector3;
use super::{HasFaces, HasBrushes, HasVisData};
use crate::coords::CoordSystem;

/// A node in a BSP tree.
/// Either has two children *or* a leaf entry.
#[derive(Debug, Clone)]
pub struct BSPNode {
    pub plane_idx: u32,
    pub min: Vector3<i32>,
    pub max: Vector3<i32>,
    pub value: BSPNodeValue
}

#[derive(Debug, Clone)]
pub enum BSPNodeValue {
    Leaf (BSPLeaf),
    Children (Box<BSPNode>, Box<BSPNode>)
}

/// A leaf in a BSP tree.
/// Will be under a `BSPNode`, min and max values are stored there.
#[derive(Debug, Clone)]
pub struct BSPLeaf {
    pub cluster_id: u32,
    pub area: i32,
    pub faces_idx: Box<[u32]>,
    pub brushes_idx: Box<[u32]>,
}

pub trait HasBSPTree<S: CoordSystem>: HasFaces<S> + HasBrushes<S> + HasVisData {
    fn get_bsp_root<'a>(&'a self) -> &'a BSPNode;
}