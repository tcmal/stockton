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

//! Renders ./example.bsp

use stockton_input::{Axis, InputManager};
#[macro_use]
extern crate stockton_input_codegen;
use std::collections::BTreeMap;
use winit::{event::Event, event_loop::EventLoop, window::WindowBuilder};

use stockton_levels::{prelude::*, q3::Q3BSPFile};
use stockton_render::{
    do_render_system, window::process_window_events_system, Renderer, WindowEvent,
};
use stockton_types::Session;

#[derive(InputManager, Default, Clone, Debug)]
struct MovementInputs {
    #[axis]
    x: Axis,

    #[axis]
    y: Axis,

    #[axis]
    z: Axis,
}

fn main() {
    // Initialise logger
    simple_logger::SimpleLogger::new()
        .with_level(log::LevelFilter::Debug)
        .init()
        .unwrap();

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

    // Create the renderer
    let (renderer, tx) = Renderer::new(&window, &bsp);
    let new_control_flow = renderer.update_control_flow.clone();

    // Create the input manager
    let manager = {
        let actions = BTreeMap::new();
        // TODO: An actual control schema

        MovementInputsManager::new(actions)
    };

    // Load everything into the session
    let mut session = Session::new(
        move |resources| {
            resources.insert(renderer);
            resources.insert(bsp);
            resources.insert(manager);
        },
        move |schedule| {
            schedule
                .add_system(process_window_events_system::<MovementInputsManager>())
                .add_thread_local(do_render_system::<Q3BSPFile<VulkanSystem>>());
        },
    );

    // Done loading - This is our main loop.
    // It just communicates events to the session and continuously ticks
    event_loop.run(move |event, _, flow| {
        match event {
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => session.do_update(),
            _ => {
                if let Some(we) = WindowEvent::from(&event) {
                    tx.send(we).unwrap();
                }
            }
        }

        // Update the control flow if the session has requested it.
        {
            let new_control_flow = new_control_flow.read().unwrap();
            if *new_control_flow != *flow {
                *flow = *new_control_flow;
            }
        };
    });
}
