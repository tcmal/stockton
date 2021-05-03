//! Parses visdata from Q3 BSPs.

use bitvec::prelude::*;
use std::vec::IntoIter;

use super::file::Q3BspFile;
use crate::coords::CoordSystem;
use crate::helpers::slice_to_i32;
use crate::traits::visdata::*;
use crate::types::{ParseError, Result};

/// Stores cluster-to-cluster visibility information.
pub fn from_data(data: &[u8]) -> Result<Box<[BitBox<Local, u8>]>> {
    if data.len() < 8 {
        return Err(ParseError::Invalid);
    }

    let n_vecs = slice_to_i32(&data[0..4]) as usize;
    let size_vecs = slice_to_i32(&data[4..8]) as usize;

    if data.len() - 8 != (n_vecs * size_vecs) {
        return Err(ParseError::Invalid);
    }

    let mut vecs = Vec::with_capacity(n_vecs);
    for n in 0..n_vecs {
        let offset = 8 + (n * size_vecs);
        let slice = &data[offset..offset + size_vecs];
        vecs.push(BitBox::from_slice(slice));
    }

    Ok(vecs.into_boxed_slice())
}

impl<T: CoordSystem> HasVisData for Q3BspFile<T> {
    type VisibleIterator = IntoIter<ClusterId>;

    fn all_visible_from(&self, from: ClusterId) -> Self::VisibleIterator {
        let mut visible = vec![];

        for (idx, val) in self.visdata[from as usize].iter().enumerate() {
            if *val {
                visible.push(idx as u32);
            }
        }

        visible.into_iter()
    }

    fn cluster_visible_from(&self, from: ClusterId, dest: ClusterId) -> bool {
        self.visdata[from as usize][dest as usize]
    }
}
