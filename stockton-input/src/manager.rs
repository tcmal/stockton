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

/// A thing that pressing a button can do to an input.
#[derive(Debug, Clone, Copy)]
pub enum InputMutation {
    MapToButton,
    NegativeAxis,
    PositiveAxis,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8)
}

impl MouseButton {
    fn keycode(&self) -> u32 {
        u32::MAX - match self {
            MouseButton::Left => 0,
            MouseButton::Right => 1,
            MouseButton::Middle => 2,
            MouseButton::Other(x) => *x as u32
        }
    }
}

/// A key being pressed or released
#[derive(Debug, Clone, Copy)]
pub enum Action {
    KeyPress(u32),
    KeyRelease(u32),
    MousePress(MouseButton),
    MouseRelease(MouseButton)
}

impl Action {
    pub fn keycode(&self) -> u32 {
        match self {
            Action::KeyPress(x) => *x,
            Action::KeyRelease(x) => *x,
            Action::MousePress(x) => x.keycode(),
            Action::MouseRelease(x) => x.keycode(),

        }
    }
    pub fn is_down(&self) -> bool {
        match self {
            Action::KeyPress(_) => true,
            Action::MousePress(_) => true,
            Action::KeyRelease(_) => false,
            Action::MouseRelease(_) => false,
        }
    }
}

pub trait InputManager {
    type Inputs;

    fn handle_frame<'a, X: IntoIterator<Item = &'a Action>>(&mut self, actions: X);
    fn get_inputs(&self) -> &Self::Inputs;
}
