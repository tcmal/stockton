// Copyright (C) Oscar Shrimpton 2019

// This program is free software: you can redistribute it and/or modify it
// under the terms of the GNU General Public License as published by the Free
// Software Foundation, either version 3 of the License, or (at your option)
// any later version.

// This program is distributed in the hope that it will be useful, but WITHOUT
// ANY WARRANTY; without even the implied warranty of MERCHANTABILITY or
// FITNESS FOR A PARTICULAR PURPOSE.  See the GNU General Public License for
// more details.

// You should have received a copy of the GNU General Public License along
// with this program.  If not, see <http://www.gnu.org/licenses/>.

//! Renders ./example.bsp

extern crate log;
extern crate simple_logger;
extern crate stockton_levels;
extern crate stockton_render;
extern crate stockton_types;
extern crate winit;

use std::f32::consts::PI;
use std::time::SystemTime;

use winit::{
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

use stockton_levels::{prelude::*, q3::Q3BSPFile};
use stockton_render::Renderer;
use stockton_types::{Vector2, Vector3, World};

/// Movement speed, world units per second
const SPEED: f32 = 100.0;

/// Pixels required to rotate 90 degrees
const PIXELS_PER_90D: f32 = 100.0;

/// Sensitivity, derived from above
const SENSITIVITY: f32 = PI / (2.0 * PIXELS_PER_90D);

#[derive(Debug)]
struct KeyState {
    pub up: bool,
    pub left: bool,
    pub right: bool,
    pub down: bool,
    pub inwards: bool,
    pub out: bool,
}

impl KeyState {
    pub fn new() -> KeyState {
        KeyState {
            up: false,
            left: false,
            right: false,
            down: false,
            inwards: false,
            out: false,
        }
    }

    /// Helper function to get our movement request as a normalized vector
    pub fn as_vector(&self) -> Vector3 {
        let mut vec = Vector3::new(0.0, 0.0, 0.0);

        if self.up {
            vec.y = 1.0;
        } else if self.down {
            vec.y = -1.0;
        }

        if self.right {
            vec.x = 1.0;
        } else if self.left {
            vec.x = -1.0;
        }

        if self.inwards {
            vec.z = 1.0;
        } else if self.out {
            vec.z = -1.0;
        }

        vec
    }
}

fn main() {
    // Initialise logger
    simple_logger::init_with_level(log::Level::Debug).unwrap();

    // Make a window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new().build(&event_loop).unwrap();

    if window.set_cursor_grab(true).is_err() {
        println!("warning: cursor not grabbed");
    }
    window.set_cursor_visible(false);

    // Parse the map file and swizzle the co-ords
    let data = include_bytes!("../data/newtest.bsp")
        .to_vec()
        .into_boxed_slice();
    let bsp: Result<Q3BSPFile<Q3System>, stockton_levels::types::ParseError> =
        Q3BSPFile::parse_file(&data);
    let bsp: Q3BSPFile<Q3System> = bsp.unwrap();
    let bsp: Q3BSPFile<VulkanSystem> = bsp.swizzle_to();

    // Load into a world and create the new renderer
    let world = World::new(bsp);
    let mut renderer = Renderer::new(world, &window).unwrap();

    // Done loading - This is our main loop.

    let mut last_update = SystemTime::now();
    let mut key_state = KeyState::new();
    let mut last_cursor_pos = Vector2::new(0.0, 0.0);

    // Keep rendering the world
    event_loop.run(move |event, _, flow| {
        *flow = ControlFlow::Poll;
        match event {
            Event::WindowEvent { event, .. } => match event {
                // Close when requested
                WindowEvent::CloseRequested => *flow = ControlFlow::Exit,

                WindowEvent::Resized(_) => {
                    unsafe { renderer.context.handle_surface_change().unwrap() };
                }

                // Update our keystates
                WindowEvent::KeyboardInput { input, .. } => match input.scancode {
                    // A
                    30 => key_state.left = input.state == ElementState::Pressed,
                    // D
                    32 => key_state.right = input.state == ElementState::Pressed,
                    // W (in)
                    17 => key_state.inwards = input.state == ElementState::Pressed,
                    // S (out)
                    31 => key_state.out = input.state == ElementState::Pressed,
                    // Space (up)
                    57 => key_state.up = input.state == ElementState::Pressed,
                    // Ctrl (down)
                    42 => key_state.down = input.state == ElementState::Pressed,
                    _ => (),
                },

                // Look around with mouse
                WindowEvent::CursorMoved { position, .. } => {
                    // Don't do anything on the first frame
                    if last_cursor_pos.x != 0.0 || last_cursor_pos.y == 0.0 {
                        // Figure out how much to rotate by
                        let x_offset = (position.x as f32 - last_cursor_pos.x) * SENSITIVITY;
                        let y_offset = (position.y as f32 - last_cursor_pos.y) * SENSITIVITY;

                        // Rotate
                        renderer
                            .context
                            .rotate(Vector3::new(-y_offset, x_offset, 0.0));
                    }

                    // For tracking how much the mouse has moved
                    last_cursor_pos.x = position.x as f32;
                    last_cursor_pos.y = position.y as f32;
                }
                _ => (),
            },

            // Nothing new happened
            Event::MainEventsCleared => {
                // Draw as many frames as we can
                // This isn't ideal, but it'll do for now.
                window.request_redraw()
            }

            // Redraw - either from above or resizing, etc
            Event::RedrawRequested(_) => {
                // Time since last frame drawn. Again, not ideal.
                let delta = last_update.elapsed().unwrap().as_secs_f32();
                last_update = SystemTime::now();

                // Move our camera if needed
                let delta_pos = key_state.as_vector() * delta * SPEED;
                if delta_pos.x != 0.0 || delta_pos.y != 0.0 || delta_pos.z != 0.0 {
                    renderer.context.move_camera_relative(delta_pos);
                }

                // Render the frame
                renderer.render_frame().unwrap()
            }
            _ => (),
        }
    });
}
