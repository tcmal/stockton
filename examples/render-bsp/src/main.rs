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
extern crate stockton_bsp;
extern crate stockton_render;
extern crate winit;
extern crate simple_logger;

use std::fs::File;
use std::io::Read;

use stockton_bsp::BSPFile;
use stockton_types::World;
use stockton_render::Renderer;

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};

fn main() {
	simple_logger::init().unwrap();

	// Load the world and renderer
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new().build(&event_loop).unwrap();
	let data = include_bytes!("../data/test.bsp");
	let bsp = BSPFile::from_buffer(data).unwrap();
	println!("{:?}", bsp);
	let world = World::new(bsp).unwrap();
	let mut renderer = Renderer::new(world, &window).unwrap();

	// Keep rendering the world
	event_loop.run(move |event, _, flow| {
		*flow = ControlFlow::Poll;
		
		match event {
			// TODO: Handle resize
			Event::WindowEvent {
				event: WindowEvent::CloseRequested,
				..
			} => {
				*flow = ControlFlow::Exit
			},

			Event::MainEventsCleared => {
				window.request_redraw()
			},
			Event::RedrawRequested(_) => {		
				renderer.render_frame().unwrap()
			}
			_ => ()
		}
	});
}
