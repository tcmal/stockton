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

use stockton_bsp::BSPFile;
use stockton_render::Renderer;
use stockton_types::World;

use std::sync::{Arc,RwLock};

fn main() {

	simple_logger::init().unwrap();
  
	// Parse the BSP file.
	let data = include_bytes!("../13power.bsp");
	let bsp = BSPFile::from_buffer(data).unwrap();

	// Load it into a world.
	// None of the entities are mapped for simplicity.
	let world = Arc::new(RwLock::new(World::new(bsp, |_| {
		None
	}).unwrap()));

	// Create the renderer.
	let mut renderer = Renderer::new(world).unwrap();

    loop {
    	// TODO: Poll Window events
    	// TODO: Handle resize
    	// TODO: Simulate world

    	renderer.render_frame().unwrap();
	}
}
