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

use super::Q3BSPFile;
use crate::traits::textures::*;
use crate::helpers::slice_to_u32;
use crate::types::{Result, ParseError};
use crate::coords::CoordSystem;

const TEXTURE_LUMP_SIZE: usize = 64 + 4 + 4;

/// Try to parse the given buffer as an entities lump.
/// # Format
/// Each entity is:
/// string[64] name     Texture name.
/// int flags           Surface flags.
/// int contents        Content flags.
/// Length of entities is total lump size / TEXTURE_LUMP_SIZE (64 + 4 + 4)
pub fn from_data(lump: &[u8]) -> Result<Box<[Texture]>> {
    if lump.is_empty() || lump.len() % TEXTURE_LUMP_SIZE != 0 {
        return Err(ParseError::Invalid);
    }
    let length = lump.len() / TEXTURE_LUMP_SIZE;

    let mut textures = Vec::with_capacity(length);
    for n in 0..length {
        let offset = n * TEXTURE_LUMP_SIZE;
        textures.push(Texture {
            name: str::from_utf8(&lump[offset..offset + 64]).map_err(|_| ParseError::Invalid)?.trim_matches('\0').to_owned(),
            surface: SurfaceFlags::from_bits_truncate(slice_to_u32(&lump[offset + 64..offset + 68])),
            contents: ContentsFlags::from_bits_truncate(slice_to_u32(&lump[offset + 68..offset + 72])),
        });
    }

    Ok(textures.into_boxed_slice())
}

impl<T: CoordSystem> HasTextures for Q3BSPFile<T> {
    type TexturesIter<'a> = std::slice::Iter<'a, Texture>;

    fn textures_iter<'a>(&'a self) -> Self::TexturesIter<'a> {
        self.textures.iter()
    }

    fn get_texture<'a>(&'a self, idx: u32) -> &'a Texture {
        &self.textures[idx as usize]
    }
}

#[test]
fn textures_single_texture() {
    let buf: &[u8] = &[
        b'T', b'E', b'S', b'T', b' ', b'T', b'E', b'X', b'T', b'U', b'R', b'E', 0x00, 0x00, 0x00,
        0x00, // name (padded to 64 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x43, 0x00, 0x04, 0x00, // surface flags
        0x09, 0x00, 0x00, 0x00, // contents flags
    ];

    let lump = from_data(buf).unwrap();

    assert_eq!(lump.len(), 1);

    assert_eq!(lump[0].name, "TEST TEXTURE");
    assert_eq!(
        lump[0].surface,
        SurfaceFlags::NO_DAMAGE | SurfaceFlags::SLICK | SurfaceFlags::FLESH | SurfaceFlags::DUST
    );
    assert_eq!(
        lump[0].contents,
        ContentsFlags::SOLID | ContentsFlags::LAVA
    );
}

#[test]
fn textures_multiple_textures() {
    let buf: &[u8] = &[
        b'T', b'E', b'S', b'T', b' ', b'T', b'E', b'X', b'T', b'U', b'R', b'E', b'1', 0x00, 0x00,
        0x00, // name (padded to 64 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x43, 0x00, 0x04, 0x00, // surface flags
        0x09, 0x00, 0x00, 0x00, // contents flags
        b'T', b'E', b'S', b'T', b' ', b'T', b'E', b'X', b'T', b'U', b'R', b'E', b'2', 0x00, 0x00,
        0x00, // name (padded to 64 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x2c, 0x10, 0x00, 0x00, // surface flags
        0x01, 0x00, 0x00, 0x00, // contents flags
        b'T', b'E', b'S', b'T', b' ', b'T', b'E', b'X', b'T', b'U', b'R', b'E', b'3', 0x00, 0x00,
        0x00, // name (padded to 64 bytes)
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00,
        0x00, 0x00, 0x00, 0x00, 0x0a, 0x00, 0x00, // surface flags
        0x41, 0x00, 0x00, 0x00, // contents flags
    ];

    let lump = from_data(buf).unwrap();

    assert_eq!(lump.len(), 3);

    assert_eq!(lump[0].name, "TEST TEXTURE1");
    assert_eq!(lump[1].name, "TEST TEXTURE2");
    assert_eq!(lump[2].name, "TEST TEXTURE3");

    assert_eq!(
        lump[0].surface,
        SurfaceFlags::NO_DAMAGE | SurfaceFlags::SLICK | SurfaceFlags::FLESH | SurfaceFlags::DUST
    );
    assert_eq!(
        lump[1].surface,
        SurfaceFlags::METAL_STEPS
            | SurfaceFlags::NO_MARKS
            | SurfaceFlags::LADDER
            | SurfaceFlags::SKY
    );
    assert_eq!(
        lump[2].surface,
        SurfaceFlags::POINT_LIGHT | SurfaceFlags::SKIP
    );

    assert_eq!(
        lump[0].contents,
        ContentsFlags::SOLID | ContentsFlags::LAVA
    );
    assert_eq!(lump[1].contents, ContentsFlags::SOLID);
    assert_eq!(
        lump[2].contents,
        ContentsFlags::SOLID | ContentsFlags::FOG
    );
}
