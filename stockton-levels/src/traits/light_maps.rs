use std::fmt;

use crate::types::Rgb;

/// Stores light map textures that help make surface lighting more realistic
#[derive(Clone)]
pub struct LightMap {
    pub map: [[Rgb; 128]; 128],
}

impl PartialEq for LightMap {
    fn eq(&self, other: &LightMap) -> bool {
        for x in 0..128 {
            for y in 0..128 {
                if self.map[x][y] != other.map[x][y] {
                    return false;
                }
            }
        }
        true
    }
}

impl fmt::Debug for LightMap {
    // rust can't derive debug for 3d arrays so done manually
    // \_( )_/
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "LightMap {{ map: [")?;
        for c in self.map.iter() {
            write!(f, "[")?;
            for x in c.iter() {
                write!(f, "{:?}, ", x)?;
            }
            write!(f, "], ")?;
        }
        write!(f, "}}")
    }
}

pub trait HasLightMaps {
    type LightMapsIter<'a>: Iterator<Item = &'a LightMap>;

    fn lightmaps_iter(&self) -> Self::LightMapsIter<'_>;
    fn get_lightmap(&self, index: u32) -> &LightMap;
}
