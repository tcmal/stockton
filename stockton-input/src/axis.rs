/*
 * Copyright (C) Oscar Shrimpton 2020
 *
 * This program is free software: you can redistribute it and/or modify it
 * under the terms of the GNU General Public License as published by the Free
 * Software Foundation, either version 3 of the License, or (at your option)
 * any later version.
 *
 * This program is distributed in the hope that it will be useful, but WITHOUT
 * ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
 * FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
 * more details.
 *
 * You should have received a copy of the GNU General Public License along
 * with this program.  If not, see <http://www.gnu.org/licenses/>.
 */

use std::fmt::Debug;
use std::ops::{Deref, DerefMut};

#[derive(Debug, Clone)]
/// A linear axis, usually with a value from -1 to 1.
pub struct Axis(i8);

impl Axis {
    /// Get a new instance with the value set to zero
    pub fn zero() -> Self {
        Axis(0)
    }

    /// Get the normalized value, ie always positive.
    pub fn normalized(&self) -> i8 {
        if self.0 < 0 {
            -self.0
        } else {
            self.0
        }
    }

    pub fn modify(&mut self, val: i8) {
        self.0 += val
    }
}

impl Default for Axis {
    fn default() -> Self {
        Self::zero()
    }
}

impl Deref for Axis {
    type Target = i8;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl DerefMut for Axis {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
