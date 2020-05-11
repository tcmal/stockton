// Copyright (C) 2019 Oscar Shrimpton
//
// This file is part of stockton-bsp.
//
// rust-bsp is free software: you can redistribute it and/or modify
// it under the terms of the GNU General Public License as published by
// the Free Software Foundation, either version 3 of the License, or
// (at your option) any later version.
//
// rust-bsp is distributed in the hope that it will be useful,
// but WITHOUT ANY WARRANTY; without even the implied warranty of
// MERCHANTABILITY or FITNESS FOR A PARTICULAR PURPOSE.  See the
// GNU General Public License for more details.
//
// You should have received a copy of the GNU General Public License
// along with rust-bsp.  If not, see <http://www.gnu.org/licenses/>.

//! Various types used in parsed BSP files.

use std::convert::TryInto;

/// RGBA Colour (0-255)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGBA {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl RGBA {
    /// Interpret the given bytes as an RGBA colour.
    pub fn from_bytes(bytes: [u8; 4]) -> RGBA {
        RGBA {
            r: bytes[0],
            g: bytes[1],
            b: bytes[2],
            a: bytes[3],
        }
    }

    /// Convert a slice to an RGBA colour
    /// # Panics
    /// If slice is not 4 bytes long.
    pub fn from_slice(slice: &[u8]) -> RGBA {
        RGBA::from_bytes(slice.try_into().unwrap())
    }
}

/// RGB Colour (0-255)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct RGB {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl RGB {
    /// 255, 255, 255
    pub fn white() -> RGB {
        RGB {
            r: 255,
            g: 255,
            b: 255,
        }
    }

    /// Interpret the given bytes as an RGB colour.
    pub fn from_bytes(bytes: [u8; 3]) -> RGB {
        RGB {
            r: bytes[0],
            g: bytes[1],
            b: bytes[2],
        }
    }

    /// Convert a slice to an RGB colour
    /// # Panics
    /// If slice is not 3 bytes long.
    pub fn from_slice(slice: &[u8]) -> RGB {
        RGB::from_bytes(slice.try_into().unwrap())
    }
}

#[derive(Debug)]
/// An error encountered while parsing.
pub enum ParseError {
    Unsupported,
    Invalid
}

/// Standard result type.
pub type Result<T> = std::result::Result<T, ParseError>;