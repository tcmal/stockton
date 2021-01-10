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

#[macro_use]
extern crate stockton_input_codegen;

#[macro_use]
extern crate legion;

use std::collections::BTreeMap;
use winit::{event::Event, event_loop::EventLoop, window::WindowBuilder};

use stockton_contrib::delta_time::*;
use stockton_contrib::flycam::*;

use stockton_input::{Axis, InputManager, Mouse};
use stockton_levels::{prelude::*, q3::Q3BSPFile};

use stockton_render::systems::*;
use stockton_render::{Renderer, UIState, WindowEvent};

use stockton_types::components::{CameraSettings, Transform};
use stockton_types::{Session, Vector3};

#[derive(InputManager, Default, Clone, Debug)]
struct MovementInputs {
    #[axis]
    x: Axis,

    #[axis]
    y: Axis,

    #[axis]
    z: Axis,
}

impl FlycamInput for MovementInputs {
    fn get_x_axis(&self) -> &Axis {
        &self.x
    }
    fn get_y_axis(&self) -> &Axis {
        &self.y
    }
    fn get_z_axis(&self) -> &Axis {
        &self.z
    }
}

#[system]
fn hello_world(#[resource] ui: &mut UIState, #[state] name: &mut String, #[state] age: &mut f32) {
    let ui = ui.ui();
    ui.heading("ABCDEFGHIJKLMNOPQRSTUVWXYZ");
    // ui.horizontal(|ui| {
    //     ui.label("Your name: ");
    //     ui.text_edit(name);
    // });
    // ui.add(egui::Slider::f32(age, 0.0..=120.0).text("age"));
    // if ui.button("Click each year").clicked {
    //     *age += 1.0;
    // }
    // ui.label(format!("Hello '{}', age {}", name, age));
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
        use stockton_input::InputMutation::*;
        use MovementInputsFields::*;

        let mut actions = BTreeMap::new();

        actions.insert(17, (Z, PositiveAxis)); // W
        actions.insert(30, (X, NegativeAxis)); // A
        actions.insert(31, (Z, NegativeAxis)); // S
        actions.insert(32, (X, PositiveAxis)); // D
        actions.insert(29, (Y, NegativeAxis)); // Ctrl
        actions.insert(57, (Y, PositiveAxis)); // Space

        MovementInputsManager::new(actions)
    };

    // Load everything into the session
    let mut session = Session::new(
        move |resources| {
            resources.insert(UIState::new(&renderer));
            resources.insert(renderer);
            resources.insert(bsp);
            resources.insert(manager);
            resources.insert(Timing::default());
            resources.insert(Mouse::default());
        },
        move |schedule| {
            schedule
                .add_system(update_deltatime_system())
                .add_system(process_window_events_system::<MovementInputsManager>())
                .flush()
                .add_system(hello_world_system("".to_string(), 0.0))
                .add_system(flycam_move_system::<MovementInputsManager>())
                .flush()
                .add_system(calc_vp_matrix_system())
                .add_thread_local(do_render_system::<Q3BSPFile<VulkanSystem>>());
        },
    );

    // Add our player entity
    let _player = session.world.push((
        Transform {
            position: Vector3::new(0.0, 0.0, 0.0),
            rotation: Vector3::new(0.0, 0.0, 0.0),
        },
        CameraSettings {
            far: 1024.0,
            fov: 90.0,
            near: 0.1,
        },
        FlycamControlled::new(512.0, 400.0),
    ));

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
