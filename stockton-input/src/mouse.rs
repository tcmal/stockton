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

use stockton_types::Vector2;

#[derive(Debug, Clone)]
pub struct Mouse {
    pub abs: Vector2,
    pub delta: Vector2,
}

impl Default for Mouse {
    fn default() -> Self {
        Mouse {
            abs: Vector2::zeros(),
            delta: Vector2::zeros(),
        }
    }
}

impl Mouse {
    pub fn handle_frame(&mut self, new: Vector2) {
        self.delta = new - self.abs;
        self.abs = new;
    }
}
