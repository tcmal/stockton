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
extern crate rand;
extern crate image;

use stockton_render::draw::{RenderingContext, UVPoint};
use stockton_types::{Vector2, Vector3};
use image::load_from_memory;

use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder
};

fn main() {

	simple_logger::init().unwrap();

	// Create the renderer.
	let event_loop = EventLoop::new();
	let window = WindowBuilder::new().build(&event_loop).unwrap();
	let mut ctx = RenderingContext::new(&window).unwrap();

	// Load 2 test textures
	ctx.add_texture(
		load_from_memory(include_bytes!("../data/test1.png"))
			.expect("Couldn't load test texture 1")
			.into_rgba())
	.unwrap();
	ctx.add_texture(
		load_from_memory(include_bytes!("../data/test2.png"))
			.expect("Couldn't load test texture 2")
			.into_rgba())
	.unwrap();

	// First quad with test1
	ctx.vert_buffer[0] = UVPoint(Vector2::new(-1.0, -1.0), Vector3::new(1.0, 0.0, 0.0), Vector2::new(0.0, 0.0), 0);
	ctx.vert_buffer[1] = UVPoint(Vector2::new(0.0, -1.0), Vector3::new(0.0, 1.0, 0.0), Vector2::new(1.0, 0.0), 0);
	ctx.vert_buffer[2] = UVPoint(Vector2::new(0.0, 0.0), Vector3::new(0.0, 0.0, 1.0), Vector2::new(1.0, 1.0), 0);
	ctx.vert_buffer[3] = UVPoint(Vector2::new(-1.0, 0.0), Vector3::new(1.0, 0.0, 1.0), Vector2::new(0.0, 1.0), 0);

	ctx.index_buffer[0] = (0, 1, 2);
	ctx.index_buffer[1] = (0, 2, 3);

	// Second quad with test2
	ctx.vert_buffer[4] = UVPoint(Vector2::new(0.0, -1.0), Vector3::new(1.0, 0.0, 0.0), Vector2::new(0.0, 0.0), 1);
	ctx.vert_buffer[5] = UVPoint(Vector2::new(1.0, -1.0), Vector3::new(0.0, 1.0, 0.0), Vector2::new(1.0, 0.0), 1);
	ctx.vert_buffer[6] = UVPoint(Vector2::new(1.0, 0.0), Vector3::new(0.0, 0.0, 1.0), Vector2::new(1.0, 1.0), 1);
	ctx.vert_buffer[7] = UVPoint(Vector2::new(0.0, 0.0), Vector3::new(1.0, 0.0, 1.0), Vector2::new(0.0, 1.0), 1);

	ctx.index_buffer[2] = (4, 5, 6);
	ctx.index_buffer[3] = (4, 7, 6);

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

			Event::WindowEvent {
				event: WindowEvent::CursorMoved {
					position,
					..
				},
				..
			} => {
				let win_size = window.inner_size();
				let mouse_x: f32 = ((position.x / win_size.width as f64) * 2.0 - 1.0) as f32;
				let mouse_y: f32 = ((position.y / win_size.height as f64) * 2.0 - 1.0) as f32;

				// Move a vertex from each quad
				ctx.vert_buffer[2] = UVPoint(Vector2::new(mouse_x, mouse_y), Vector3::new(1.0, 0.0, 0.0), Vector2::new(1.0, 1.0), 0);
				ctx.vert_buffer[7] = UVPoint(Vector2::new(mouse_x, mouse_y), Vector3::new(1.0, 0.0, 0.0), Vector2::new(0.0, 1.0), 1);
			}

			Event::MainEventsCleared => {
				window.request_redraw()
			},
			Event::RedrawRequested(_) => {		
				ctx.draw_vertices().unwrap();
			}
			_ => ()
		}
	});
}
