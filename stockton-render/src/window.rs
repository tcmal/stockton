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

use crate::Renderer;

use winit::event::Event as WinitEvent;

pub struct WindowEvent {}

impl WindowEvent {
    pub fn from(_winit_event: &WinitEvent<()>) -> WindowEvent {
        // TODO
        WindowEvent {}
    }
}

#[system]
/// A system to process the window events sent to renderer by the winit event loop.
pub fn process_window_events(#[resource] _renderer: &mut Renderer<'static>) {
    println!("processing window events...");
}
