//! Renders ./example.bsp geometry: (), texture_idx: ()  geometry: (), texture_idx: ()

#[macro_use]
extern crate stockton_input_codegen;

#[macro_use]
extern crate legion;

use std::{
    collections::BTreeMap,
    path::Path,
    sync::{Arc, RwLock},
};

use stockton_contrib::{delta_time::*, flycam::*};
use stockton_input::{Axis, InputManager, Mouse};
use stockton_levels::{
    parts::data::{Geometry, Vertex},
    types::Rgba,
};
use stockton_render::{
    level::{LevelDrawPass, LevelDrawPassConfig},
    ui::UiDrawPass,
    window::{process_window_events_system, UiState, WindowEvent, WindowFlow},
};
use stockton_skeleton::{
    draw_passes::ConsDrawPass, error::full_error_display, texture::resolver::FsResolver, Renderer,
};
use stockton_types::{
    components::{CameraSettings, Transform},
    Session, Vector2, Vector3,
};

use anyhow::{Context, Result};
use egui::{containers::CentralPanel, Frame};
use log::warn;
use winit::{
    event::Event,
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod level;
use level::*;

type Dp<'a> = ConsDrawPass<LevelDrawPass<'a, DemoLevel>, UiDrawPass<'a>>;

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
fn hello_world(#[resource] ui: &mut UiState) {
    CentralPanel::default()
        .frame(Frame::none())
        .show(ui.ctx(), |ui| {
            ui.heading("Hello, World!");
        });
}

fn main() {
    if let Err(err) = try_main() {
        eprintln!("{}", full_error_display(err));
    }
}

fn try_main() -> Result<()> {
    // Initialise logger
    simplelog::TermLogger::init(
        log::LevelFilter::Debug,
        simplelog::ConfigBuilder::new()
            .set_max_level(log::LevelFilter::Debug)
            .set_thread_mode(simplelog::ThreadLogMode::Names)
            .build(),
        simplelog::TerminalMode::Stderr,
        simplelog::ColorChoice::Auto,
    )
    .context("Error initialising logger")?;

    // Make a window
    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .build(&event_loop)
        .context("Error creating window")?;

    if window.set_cursor_grab(true).is_err() {
        warn!("warning: cursor not grabbed");
    }
    window.set_cursor_visible(false);

    // TODO: Parse the map file
    let map = Arc::new(RwLock::new(DemoLevel {
        faces: vec![Face {
            geometry: Geometry::Vertices(
                Vertex {
                    position: Vector3::new(-128.0, 128.0, 128.0),
                    tex: Vector2::new(0.0, 0.0),
                    color: Rgba::from_slice(&[0, 0, 0, 1]),
                },
                Vertex {
                    position: Vector3::new(-128.0, -128.0, 128.0),
                    tex: Vector2::new(0.0, 1.0),
                    color: Rgba::from_slice(&[0, 0, 0, 1]),
                },
                Vertex {
                    position: Vector3::new(128.0, 128.0, 128.0),
                    tex: Vector2::new(1.0, 0.0),
                    color: Rgba::from_slice(&[0, 0, 0, 1]),
                },
            ),
            texture_idx: 0,
        }]
        .into_boxed_slice(),
        textures: vec![Texture {
            name: "example_texture".to_string(),
        }]
        .into_boxed_slice(),
    }));

    // Create the UI State
    let ui = UiState::default();

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
    let mut session = Session::new(move |schedule| {
        schedule
            .add_system(update_deltatime_system())
            .add_system(process_window_events_system::<MovementInputsManager>())
            .flush()
            .add_system(hello_world_system())
            .add_system(flycam_move_system::<MovementInputsManager>());
    });

    session.resources.insert(map.clone());
    session.resources.insert(manager);
    session.resources.insert(Timing::default());
    session.resources.insert(Mouse::default());
    session.resources.insert(ui);

    // Add our player entity
    let player = session.world.push((
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

    // Create the renderer
    let renderer = Renderer::<Dp<'static>>::new(
        &window,
        &mut session,
        (
            LevelDrawPassConfig {
                active_camera: player,
                tex_resolver: FsResolver::new(Path::new("./examples/render-quad/textures"), map),
            },
            (),
        ),
    )?;

    let new_control_flow = Arc::new(RwLock::new(ControlFlow::Poll));
    let (window_flow, tx) = WindowFlow::new(new_control_flow.clone());
    session.resources.insert(window_flow);

    // Populate the initial UI state
    {
        let ui = &mut session.resources.get_mut::<UiState>().unwrap();
        ui.populate_initial_state(&renderer);
    }

    let mut renderer = Some(renderer);

    // Done loading - This is our main loop.
    // It just communicates events to the session and continuously ticks
    event_loop.run(move |event, _, flow| {
        match event {
            Event::MainEventsCleared => {
                window.request_redraw();
            }
            Event::RedrawRequested(_) => {
                session.do_update();
                let r = renderer.take().unwrap();
                match r.render(&session) {
                    Ok(r) => {
                        renderer = Some(r);
                    }
                    Err(e) => {
                        println!("Error drawing: {}", full_error_display(e));

                        // TODO: Not really sound
                        *(new_control_flow.write().unwrap()) = ControlFlow::Exit;
                    }
                }
            }
            _ => {
                if let Some(we) = WindowEvent::from(&event) {
                    tx.send(we).unwrap();

                    println!("{:?}", we);
                    if let WindowEvent::SizeChanged(_, _) = we {
                        let r = renderer.take().unwrap();
                        match r.recreate_surface(&session) {
                            Ok(r) => {
                                renderer = Some(r);
                            }
                            Err(e) => {
                                println!("Error resizing: {}", full_error_display(e));

                                // TODO: Not really sound
                                *(new_control_flow.write().unwrap()) = ControlFlow::Exit;
                            }
                        }
                    }
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
