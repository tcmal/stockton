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
	let cube_points: [UVPoint; 24] = [
	  // Face 1 (front)
	  UVPoint(Vector3::new(0.0, 0.0, 0.0), Vector2::new(0.0, 1.0), 0), /* bottom left */
	  UVPoint(Vector3::new(0.0, 1.0, 0.0), Vector2::new(0.0, 0.0), 0), /* top left */
	  UVPoint(Vector3::new(1.0, 0.0, 0.0), Vector2::new(1.0, 1.0), 0), /* bottom right */
	  UVPoint(Vector3::new(1.0, 1.0, 0.0), Vector2::new(1.0, 0.0), 0), /* top right */
	  // Face 2 (top)
	  UVPoint(Vector3::new(0.0, 1.0, 0.0), Vector2::new(0.0, 1.0), 0), /* bottom left */
	  UVPoint(Vector3::new(0.0, 1.0, 1.0), Vector2::new(0.0, 0.0), 0), /* top left */
	  UVPoint(Vector3::new(1.0, 1.0, 0.0), Vector2::new(1.0, 1.0), 0), /* bottom right */
	  UVPoint(Vector3::new(1.0, 1.0, 1.0), Vector2::new(1.0, 0.0), 0), /* top right */
	  // Face 3 (back)
	  UVPoint(Vector3::new(0.0, 0.0, 1.0), Vector2::new(0.0, 1.0), 1), /* bottom left */
	  UVPoint(Vector3::new(0.0, 1.0, 1.0), Vector2::new(0.0, 0.0), 1), /* top left */
	  UVPoint(Vector3::new(1.0, 0.0, 1.0), Vector2::new(1.0, 1.0), 1), /* bottom right */
	  UVPoint(Vector3::new(1.0, 1.0, 1.0), Vector2::new(1.0, 0.0), 1), /* top right */
	  // Face 4 (bottom)
	  UVPoint(Vector3::new(0.0, 0.0, 0.0), Vector2::new(0.0, 1.0), 1), /* bottom left */
	  UVPoint(Vector3::new(0.0, 0.0, 1.0), Vector2::new(0.0, 0.0), 1), /* top left */
	  UVPoint(Vector3::new(1.0, 0.0, 0.0), Vector2::new(1.0, 1.0), 1), /* bottom right */
	  UVPoint(Vector3::new(1.0, 0.0, 1.0), Vector2::new(1.0, 0.0), 1), /* top right */
	  // Face 5 (left)
	  UVPoint(Vector3::new(0.0, 0.0, 1.0), Vector2::new(0.0, 1.0), 1), /* bottom left */
	  UVPoint(Vector3::new(0.0, 1.0, 1.0), Vector2::new(0.0, 0.0), 1), /* top left */
	  UVPoint(Vector3::new(0.0, 0.0, 0.0), Vector2::new(1.0, 1.0), 1), /* bottom right */
	  UVPoint(Vector3::new(0.0, 1.0, 0.0), Vector2::new(1.0, 0.0), 1), /* top right */
	  // Face 6 (right)
	  UVPoint(Vector3::new(1.0, 0.0, 0.0), Vector2::new(0.0, 1.0), 0), /* bottom left */
	  UVPoint(Vector3::new(1.0, 1.0, 0.0), Vector2::new(0.0, 0.0), 0), /* top left */
	  UVPoint(Vector3::new(1.0, 0.0, 1.0), Vector2::new(1.0, 1.0), 0), /* bottom right */
	  UVPoint(Vector3::new(1.0, 1.0, 1.0), Vector2::new(1.0, 0.0), 0), /* top right */
	];
	let cube_indices: [(u16, u16, u16); 12] = [
		(0,  1,  2),  (2,  1,  3), // front
		(4,  5,  6),  (7,  6,  5), // top
		(10,  9,  8),  (9, 10, 11), // back
		(12, 14, 13), (15, 13, 14), // bottom
		(16, 17, 18), (19, 18, 17), // left
		(20, 21, 22), (23, 22, 21), // right
	];

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
	for (index, point) in cube_points.iter().enumerate() {
		ctx.vert_buffer[index] = *point;
	}

	for (index, value) in cube_indices.iter().enumerate() {
		ctx.index_buffer[index] = *value;
	}

	event_loop.run(move |event, _, flow| {
		*flow = ControlFlow::Poll;
		
		match event {
			Event::WindowEvent {
				event: WindowEvent::CloseRequested, 
				..
			} => {
				*flow = ControlFlow::Exit
			}

			Event::MainEventsCleared => {
				window.request_redraw()
			},
			Event::RedrawRequested(_) => {		
				if let Err(err) = ctx.draw_vertices() {
					unsafe {ctx.handle_surface_change().unwrap()};

					// If it fails twice, then panic
					ctx.draw_vertices().unwrap();
				}
			}
			_ => ()
		}
	});
}
