use crate::types::{ParseError, Result};
use std::convert::TryInto;

const MAGIC_HEADER: &[u8] = &[0x49, 0x42, 0x53, 0x50];
const HEADER_LEN: usize = 4 + 4 + (17 * 4 * 2);

/// The header found at the start of a (Q3) bsp file
#[derive(Clone, Copy, Debug)]
pub struct Header {
    pub version: u32,
    pub dir_entries: [DirEntry; 17],
}

/// A directory entry, pointing to a lump in the file
#[derive(Clone, Copy, Debug)]
pub struct DirEntry {
    /// Offset from beginning of file to start of lump
    pub offset: u32,

    /// Length of lump, multiple of 4.
    pub length: u32,
}

impl Header {
    /// Deserialise from buffer.
    /// # Format
    /// string[4] magic             Magic number. Always "IBSP".
    /// int version                 Version number. 0x2e for the BSP files distributed with Quake 3.
    /// direntry[17] direntries     Lump directory, seventeen entries.
    pub fn from(v: &[u8]) -> Result<Header> {
        if v.len() < HEADER_LEN {
            return Err(ParseError::Invalid);
        }
        let magic = &v[0..4];

        if magic != MAGIC_HEADER {
            return Err(ParseError::Invalid);
        }

        let version: &[u8; 4] = v[4..8].try_into().unwrap();

        let entries: &[u8] = &v[8..144];
        let mut dir_entries: [DirEntry; 17] = [DirEntry {
            offset: 0,
            length: 0,
        }; 17];

        for n in 0..17 {
            let base = &entries[(n * 8)..(n * 8) + 8];
            dir_entries[n] = DirEntry {
                offset: u32::from_le_bytes(base[0..4].try_into().unwrap()),
                length: u32::from_le_bytes(base[4..8].try_into().unwrap()),
            }
        }

        Ok(Header {
            version: u32::from_le_bytes(*version),
            dir_entries,
        })
    }

    /// Get the lump at given index from the buffer, with offset & length based on this directory.
    pub fn get_lump<'l>(&self, buf: &'l [u8], index: usize) -> &'l [u8] {
        let entry = self.dir_entries[index];

        &buf[entry.offset as usize..entry.offset as usize + entry.length as usize]
    }
}
