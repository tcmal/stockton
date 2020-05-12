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

use std::str;

use crate::helpers::slice_to_u32;
use crate::types::{Result, ParseError};
use crate::traits::effects::*;
use super::Q3BSPFile;
use crate::coords::CoordSystem;

/// The size of one effect definition
const EFFECT_SIZE: usize = 64 + 4 + 4;

pub fn from_data(data: &[u8], n_brushes: u32) -> Result<Box<[Effect]>> {
    if data.len() % EFFECT_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let length = data.len() / EFFECT_SIZE;

    let mut effects = Vec::with_capacity(length);
    for n in 0..length {
        let raw = &data[n * EFFECT_SIZE..(n + 1) * EFFECT_SIZE];

        let brush_idx = slice_to_u32(&raw[64..68]);
        if brush_idx >= n_brushes {
            return Err(ParseError::Invalid);
        }

        effects.push(Effect {
            name: str::from_utf8(&raw[..64]).map_err(|_| ParseError::Invalid)?.to_owned(),
            brush_idx
        });
    }

    Ok(effects.into_boxed_slice())
}


impl<T: CoordSystem> HasEffects<T> for Q3BSPFile<T> {
    type EffectsIter<'a> = std::slice::Iter<'a, Effect>;

    fn effects_iter<'a>(&'a self) -> Self::EffectsIter<'a> {
        self.effects.iter()
    }

    fn get_effect<'a>(&'a self, index: u32) -> &'a Effect {
        &self.effects[index as usize]
    }
}
