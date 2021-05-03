use super::PIXEL_SIZE;

use core::ptr::copy_nonoverlapping;
use std::convert::TryInto;

use image::RgbaImage;

/// An object that can be loaded as an image into GPU memory
pub trait LoadableImage {
    fn width(&self) -> u32;
    fn height(&self) -> u32;
    fn copy_row(&self, y: u32, ptr: *mut u8);
    unsafe fn copy_into(&self, ptr: *mut u8, row_size: usize);
}

impl LoadableImage for RgbaImage {
    fn width(&self) -> u32 {
        self.width()
    }

    fn height(&self) -> u32 {
        self.height()
    }

    fn copy_row(&self, y: u32, ptr: *mut u8) {
        let row_size_bytes = self.width() as usize * PIXEL_SIZE;
        let raw: &Vec<u8> = self.as_raw();
        let row = &raw[y as usize * row_size_bytes..(y as usize + 1) * row_size_bytes];

        unsafe {
            copy_nonoverlapping(row.as_ptr(), ptr, row.len());
        }
    }

    unsafe fn copy_into(&self, ptr: *mut u8, row_size: usize) {
        for y in 0..self.height() as usize {
            let dest_base: isize = (y * row_size).try_into().unwrap();
            self.copy_row(y as u32, ptr.offset(dest_base));
        }
    }
}
