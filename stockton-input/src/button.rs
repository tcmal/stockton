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

#[derive(Debug, Clone, PartialEq)]
/// A boolean input, with additional tracking for if it just changed state.
pub struct Button {
    /// How many of the mapped inputs are currently pressed.
    /// This is used so that holding one button, then another, then releasing the first, will keep the button down continuously as expected.
    inputs_down: u8,

    /// Whether or not the button changed state in the last batch of actions processed
    /// Note that pushing 2 buttons bound to this action one after the other won't trigger this twice.
    pub is_hot: bool,
}

impl Button {
    pub fn new() -> Self {
        Button {
            inputs_down: 0,
            is_hot: false,
        }
    }

    pub fn is_down(&self) -> bool {
        self.inputs_down > 0
    }
    pub fn is_up(&self) -> bool {
        self.inputs_down == 0
    }

    pub fn is_just_down(&self) -> bool {
        self.is_down() && self.is_hot
    }
    pub fn is_just_up(&self) -> bool {
        self.is_up() && self.is_hot
    }

    pub fn modify_inputs(&mut self, add: bool) {
        self.inputs_down = if add {
            self.inputs_down + 1
        } else {
            self.inputs_down - 1
        };

        if self.inputs_down == 1 || self.inputs_down == 0 {
            self.is_hot = true;
        }
    }

    pub fn set_not_hot(&mut self) {
        self.is_hot = false;
    }
}

impl Default for Button {
    fn default() -> Self {
        Self::new()
    }
}
