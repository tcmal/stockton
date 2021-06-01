//! Renders ./example.bsp

#[macro_use]
extern crate stockton_input_codegen;

#[macro_use]
extern crate legion;

use anyhow::{Context, Result};
use log::warn;
use std::collections::BTreeMap;
use winit::{event::Event, event_loop::EventLoop, window::WindowBuilder};

use egui::{containers::CentralPanel, Frame};
use stockton_contrib::delta_time::*;
use stockton_contrib::flycam::*;

use stockton_input::{Axis, InputManager, Mouse};
use stockton_levels::{prelude::*, q3::Q3BspFile};

use stockton_render::error::full_error_display;
use stockton_render::systems::*;
use stockton_render::{Renderer, UiState, WindowEvent};

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

    // Parse the map file and swizzle the co-ords
    let data = include_bytes!("../data/newtest.bsp")
        .to_vec()
        .into_boxed_slice();
    let bsp: Result<Q3BspFile<Q3System>, stockton_levels::types::ParseError> =
        Q3BspFile::parse_file(&data);
    let bsp: Q3BspFile<Q3System> = bsp.context("Error loading bsp")?;
    let bsp: Q3BspFile<VulkanSystem> = bsp.swizzle_to();

    // Create the UI State
    let mut ui = UiState::new();

    // Create the renderer
    let (renderer, tx) = Renderer::new(&window, &mut ui, bsp)?;
    let new_control_flow = renderer.update_control_flow.clone();

    // Populate the initial UI state
    ui.populate_initial_state(&renderer);

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
            resources.insert(ui);
            resources.insert(renderer);
            resources.insert(manager);
            resources.insert(Timing::default());
            resources.insert(Mouse::default());
        },
        move |schedule| {
            schedule
                .add_system(update_deltatime_system())
                .add_system(process_window_events_system::<
                    MovementInputsManager,
                    Q3BspFile<VulkanSystem>,
                >())
                .flush()
                .add_system(hello_world_system())
                .add_system(flycam_move_system::<MovementInputsManager>())
                .flush()
                .add_system(calc_vp_matrix_system::<Q3BspFile<VulkanSystem>>())
                .add_thread_local(do_render_system::<Q3BspFile<VulkanSystem>>());
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
                    tx.send(we).unwrap()
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
