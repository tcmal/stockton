//! Various types used in parsed BSP files.

use std::convert::TryInto;

/// RGBA Colour (0-255)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgba {
    pub r: u8,
    pub g: u8,
    pub b: u8,
    pub a: u8,
}

impl Rgba {
    /// Interpret the given bytes as an RGBA colour.
    pub fn from_bytes(bytes: [u8; 4]) -> Rgba {
        Rgba {
            r: bytes[0],
            g: bytes[1],
            b: bytes[2],
            a: bytes[3],
        }
    }

    /// Convert a slice to an RGBA colour
    /// # Panics
    /// If slice is not 4 bytes long.
    pub fn from_slice(slice: &[u8]) -> Rgba {
        Rgba::from_bytes(slice.try_into().unwrap())
    }
}

/// RGB Colour (0-255)
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Rgb {
    pub r: u8,
    pub g: u8,
    pub b: u8,
}

impl Rgb {
    /// 255, 255, 255
    pub fn white() -> Rgb {
        Rgb {
            r: 255,
            g: 255,
            b: 255,
        }
    }

    /// Interpret the given bytes as an RGB colour.
    pub fn from_bytes(bytes: [u8; 3]) -> Rgb {
        Rgb {
            r: bytes[0],
            g: bytes[1],
            b: bytes[2],
        }
    }

    /// Convert a slice to an RGB colour
    /// # Panics
    /// If slice is not 3 bytes long.
    pub fn from_slice(slice: &[u8]) -> Rgb {
        Rgb::from_bytes(slice.try_into().unwrap())
    }
}

#[derive(Debug)]
/// An error encountered while parsing.
pub enum ParseError {
    Unsupported,
    Invalid,
}

/// Standard result type.
pub type Result<T> = std::result::Result<T, ParseError>;
