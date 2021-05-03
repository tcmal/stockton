use std::collections::HashMap;
use std::str;

use super::Q3BspFile;
use crate::coords::CoordSystem;
use crate::traits::entities::*;
use crate::types::{ParseError, Result};

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

                    attrs.insert(
                        string[key_start..key_end].to_owned(),
                        string[val_start..val_end].to_owned(),
                    );
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

impl<T: CoordSystem> HasEntities for Q3BspFile<T> {
    type EntitiesIter<'a> = std::slice::Iter<'a, Entity>;

    fn entities_iter(&self) -> Self::EntitiesIter<'_> {
        self.entities.iter()
    }
}
