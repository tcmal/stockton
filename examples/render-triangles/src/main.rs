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

use stockton_render::draw::{RenderingContext, Tri2};
use stockton_types::Vector2;

use winit::{Event, WindowEvent, VirtualKeyCode, ElementState};
use rand::prelude::*;

fn main() {

	simple_logger::init().unwrap();

	// Create the renderer.
	let mut ctx = RenderingContext::new().unwrap();
	let mut rng = thread_rng();
	let mut vertices: Vec<Tri2> = Vec::new();
	let mut running = true;
	let mut vertices_dirty = false;

    while running {
    	ctx.events_loop.poll_events(|event| {
    		match event {
		    	// TODO: Handle resize
		    	Event::WindowEvent {
		    		event: WindowEvent::KeyboardInput { input,  .. },
		    		..
		    	} => match input.state {
		    		ElementState::Released => match input.virtual_keycode {
			    		Some(VirtualKeyCode::Escape) => running = false,
			    		Some(VirtualKeyCode::Space) => {
			    			vertices.push(Tri2 ([
			    				Vector2::new(
			    					rng.gen_range(-1.0, 1.0),
			    					rng.gen_range(-1.0, 1.0),
			    					),
			    				Vector2::new(
			    					rng.gen_range(-1.0, 1.0),
			    					rng.gen_range(-1.0, 1.0),
			    					),
			    				Vector2::new(
			    					rng.gen_range(-1.0, 1.0),
			    					rng.gen_range(-1.0, 1.0),
			    					)
		    				]));

			    			vertices_dirty = true;
			    		},
			    		_ => ()
			    	},
			    	_ => ()
			    }
    			_ => ()
    		}
    	});

    	if vertices_dirty {
			ctx.populate_vertices(vertices.as_slice()).unwrap();
			vertices_dirty = false;    		
    	}

    	ctx.draw_vertices().unwrap();
	}
}
