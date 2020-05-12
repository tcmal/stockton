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
use std::collections::HashMap;

use crate::types::{Result, ParseError};
use crate::traits::entities::*;
use super::Q3BSPFile;
use crate::coords::CoordSystem;

const QUOTE: u8 = b'"';
const END_BRACKET: u8 = b'}';
const START_BRACKET: u8 = b'{';

/// Internal enum to parse through the entities string.
#[derive(PartialEq, Eq)]
enum ParseState {
    InKey,
    InValue,
    AfterKey,
    InsideEntity,
    OutsideEntity,
}

/// Parse the given data as an Entities lump
pub fn from_data(data: &[u8]) -> Result<Box<[Entity]>> {
    use self::ParseState::*;

    let string = str::from_utf8(data).unwrap();

    let mut attrs = HashMap::new();
    let mut entities = Vec::new();

    let mut state = ParseState::OutsideEntity;

    let mut key_start = 0;
    let mut key_end = 0;
    let mut val_start = 0;
    let mut val_end;

    for (i, chr) in string.bytes().enumerate() {
        match chr {
            QUOTE => match state {
                InsideEntity => {
                    state = ParseState::InKey;
                    key_start = i + 1;
                }
                InKey => {
                    state = ParseState::AfterKey;
                    key_end = i;
                }
                AfterKey => {
                    state = ParseState::InValue;
                    val_start = i + 1;
                }
                InValue => {
                    state = ParseState::InsideEntity;
                    val_end = i;

                    attrs.insert(string[key_start..key_end].to_owned(), string[val_start..val_end].to_owned());
                }
                _ => {
                    return Err(ParseError::Invalid);
                }
            },
            END_BRACKET => {
                if state != InsideEntity {
                    return Err(ParseError::Invalid);
                }

                state = OutsideEntity;

                entities.push(Entity { attributes: attrs });
                attrs = HashMap::new();
            }
            START_BRACKET => {
                if state != OutsideEntity {
                    return Err(ParseError::Invalid);
                }
                state = InsideEntity;
            }
            _ => {}
        }
    }
    Ok(entities.into_boxed_slice())
}

impl<T: CoordSystem> HasEntities for Q3BSPFile<T> {
    type EntitiesIter<'a> = std::slice::Iter<'a, Entity>;

    fn entities_iter<'a>(&'a self) -> Self::EntitiesIter<'a> {
        self.entities.iter()
    }
}
