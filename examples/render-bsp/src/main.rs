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

extern crate stockton_types;
extern crate stockton_levels;
extern crate stockton_render;
extern crate winit;
extern crate simple_logger;
extern crate image;

use image::load_from_memory;
use std::time::SystemTime;

use stockton_levels::q3::Q3BSPFile;
use stockton_types::{World, Vector3};
use stockton_render::Renderer;

use winit::{
    event::{Event, WindowEvent, ElementState},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};

const SPEED: f32 = 100.0;

#[derive(Debug)]
struct KeyState {
	pub up: bool,
	pub left: bool,
	pub right: bool,
	pub down: bool,
	pub inwards: bool,
	pub out: bool
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
	simple_logger::init().unwrap();

	// Load the world and renderer
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new().build(&event_loop).unwrap();
	let data = include_bytes!("../data/test.bsp").to_vec().into_boxed_slice();
	let bsp = Q3BSPFile::new(&data).unwrap();
	
	let world = World::new(bsp);
	let mut renderer = Renderer::new(world, &window).unwrap();

	{
		renderer.context.add_texture(
			load_from_memory(include_bytes!("../../render-quad/data/test1.png"))
				.expect("Couldn't load test texture 1")
				.into_rgba()).unwrap();

		renderer.context.add_texture(
			load_from_memory(include_bytes!("../../render-quad/data/test2.png"))
				.expect("Couldn't load test texture 2")
				.into_rgba()).unwrap();
	}

	let mut last_update = SystemTime::now();
	let mut key_state = KeyState::new();

	// Keep rendering the world
	event_loop.run(move |event, _, flow| {
		*flow = ControlFlow::Poll;
		match event {
			Event::WindowEvent {
				event,
				..
			} => match event {
				WindowEvent::CloseRequested => {
					*flow = ControlFlow::Exit
				},
				WindowEvent::KeyboardInput {input, ..} => match input.scancode {
					// Left
					105 => key_state.left = input.state == ElementState::Pressed,
					// Right
					106 => key_state.right = input.state == ElementState::Pressed,
					// Up (in)
					103 => key_state.inwards = input.state == ElementState::Pressed,
					// Down (out)
					108 => key_state.out = input.state == ElementState::Pressed,
					// Space (up)
					57 => key_state.up = input.state == ElementState::Pressed,
					// Ctrl (down)
					29 => key_state.down = input.state == ElementState::Pressed,
					_ => ()
				}
				_ => ()
			},

			Event::MainEventsCleared => {
				window.request_redraw()
			},
			Event::RedrawRequested(_) => {

				let delta = last_update.elapsed().unwrap().as_secs_f32();
				last_update = SystemTime::now();

				let delta_pos = key_state.as_vector() * delta * SPEED;
				if delta_pos.x != 0.0 || delta_pos.y != 0.0 || delta_pos.z != 0.0 {
					renderer.context.move_camera(delta_pos);
					println!("camera is at {:?}", renderer.context.camera_pos());
				}

				renderer.render_frame().unwrap()
			}
			_ => ()
		}
	});
}
