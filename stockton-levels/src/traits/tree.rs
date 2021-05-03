//! Parses the BSP tree into a usable format

use super::{HasBrushes, HasFaces, HasVisData};
use crate::coords::CoordSystem;
use na::Vector3;

/// A node in a BSP tree.
/// Either has two children *or* a leaf entry.
#[derive(Debug, Clone)]
pub struct BspNode {
    pub plane_idx: u32,
    pub min: Vector3<i32>,
    pub max: Vector3<i32>,
    pub value: BspNodeValue,
}

#[derive(Debug, Clone)]
pub enum BspNodeValue {
    Leaf(BspLeaf),
    Children(Box<BspNode>, Box<BspNode>),
}

/// A leaf in a BSP tree.
/// Will be under a `BSPNode`, min and max values are stored there.
#[derive(Debug, Clone)]
pub struct BspLeaf {
    pub cluster_id: u32,
    pub area: i32,
    pub faces_idx: Box<[u32]>,
    pub brushes_idx: Box<[u32]>,
}

pub trait HasBspTree<S: CoordSystem>: HasFaces<S> + HasBrushes<S> + HasVisData {
    fn get_bsp_root(&self) -> &BspNode;
}
