// Copyright (C) 2019 Oscar Shrimpton
//
// This file is part of stockton-bsp.
//
// stockton-bsp is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// stockton-bsp is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with stockton-bsp.  If not, see <http://www.gnu.org/licenses/>.

use crate::helpers::{slice_to_u32, slice_to_vec3};
use crate::types::{Result, ParseError};
use crate::coords::CoordSystem;
use crate::traits::models::*;
use super::Q3BSPFile;

const MODEL_SIZE: usize = (4 * 3 * 2) + (4 * 4);

pub fn from_data(
    data: &[u8],
    n_faces: u32,
    n_brushes: u32,
) -> Result<Box<[Model]>> {
    if data.len() % MODEL_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let n_models = data.len() / MODEL_SIZE;

    let mut models = Vec::with_capacity(n_models);
    for n in 0..n_models {
        let raw = &data[n * MODEL_SIZE..(n + 1) * MODEL_SIZE];

        let mins = slice_to_vec3(&raw[0..12]);
        let maxs = slice_to_vec3(&raw[12..24]);

        let faces_idx = {
            let start = slice_to_u32(&raw[24..28]);
            let n = slice_to_u32(&raw[28..32]);

            if start + n > n_faces {
                return Err(ParseError::Invalid);
            }

            start..start+n
        };

        let brushes_idx = {
            let start = slice_to_u32(&raw[32..36]);
            let n = slice_to_u32(&raw[36..40]);

            if start + n > n_brushes {
                return Err(ParseError::Invalid);
            }

            start..start+n
        };

        models.push(Model {
            mins,
            maxs,
            faces_idx,
            brushes_idx,
        })
    }

    Ok(models.into_boxed_slice())
}


impl<T: CoordSystem> HasModels<T> for Q3BSPFile<T> {
    type ModelsIter<'a> = std::slice::Iter<'a, Model>;

    fn models_iter<'a>(&'a self) -> Self::ModelsIter<'a> {
        self.models.iter()
    }

    fn get_model<'a>(&'a self, index: u32) -> &'a Model {
        &self.models[index as usize]
    }
}
