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
use legion::systems::Runnable;

use stockton_input::{Action as KBAction, InputManager};

use winit::event::{ElementState, Event as WinitEvent, WindowEvent as WinitWindowEvent};
use winit::event_loop::ControlFlow;

#[derive(Debug, Clone, Copy)]
pub enum WindowEvent {
    SizeChanged,
    CloseRequested,
    KeyboardAction(KBAction),
}

impl WindowEvent {
    pub fn from(winit_event: &WinitEvent<()>) -> Option<WindowEvent> {
        // TODO
        match winit_event {
            WinitEvent::WindowEvent { event, .. } => match event {
                WinitWindowEvent::CloseRequested => Some(WindowEvent::CloseRequested),
                WinitWindowEvent::Resized(_) => Some(WindowEvent::SizeChanged),
                WinitWindowEvent::KeyboardInput { input, .. } => match input.state {
                    ElementState::Pressed => Some(WindowEvent::KeyboardAction(KBAction::KeyPress(
                        input.scancode,
                    ))),
                    ElementState::Released => Some(WindowEvent::KeyboardAction(
                        KBAction::KeyRelease(input.scancode),
                    )),
                },
                _ => None,
            },
            _ => None,
        }
    }
}

#[system]
/// A system to process the window events sent to renderer by the winit event loop.
pub fn _process_window_events<T: 'static + InputManager>(
    #[resource] renderer: &mut Renderer<'static>,
    #[resource] manager: &mut T,
    #[state] actions_buf: &mut Vec<KBAction>,
) {
    let mut actions_buf_cursor = 0;

    while let Ok(event) = renderer.window_events.try_recv() {
        match event {
            WindowEvent::SizeChanged => renderer.resize(),
            WindowEvent::CloseRequested => {
                let mut flow = renderer.update_control_flow.write().unwrap();
                // TODO: Let everything know this is our last frame
                *flow = ControlFlow::Exit;
            }
            WindowEvent::KeyboardAction(action) => {
                if actions_buf_cursor >= actions_buf.len() {
                    actions_buf.push(action);
                } else {
                    actions_buf[actions_buf_cursor] = action;
                }
                actions_buf_cursor += 1;
            }
        };
    }

    manager.handle_frame(&actions_buf[0..actions_buf_cursor]);
}

pub fn process_window_events_system<T: 'static + InputManager>() -> impl Runnable {
    _process_window_events_system::<T>(Vec::with_capacity(4))
}
