//! Renders ./example.bsp geometry: (), texture_idx: ()  geometry: (), texture_idx: ()

extern crate gfx_hal as hal;

#[macro_use]
extern crate legion;

use anyhow::{Context, Result};
use log::warn;
use stockton_skeleton::{error::full_error_display, Renderer, Session};
use winit::{
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};

mod draw_pass;
mod system;
use draw_pass::*;
use system::*;

/// Alias for our drawpass
type Dp<'a> = ExampleDrawPass<'a>;

fn main() {
    // Wrap for full error display
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

    // Grab cursor
    window.set_cursor_visible(false);
    if window.set_cursor_grab(true).is_err() {
        warn!("warning: cursor not grabbed");
    }

    // Our game world
    let mut session = Session::new(move |schedule| {
        schedule.add_system(mutate_state_system());
    });

    // An entity to keep track of our state
    let state_ent = session.world.push((ExampleState::default(),));

    // Create the renderer
    let renderer =
        Renderer::<Dp<'static>>::new(&window, &mut session, ExampleDrawPassConfig { state_ent })?;

    // We'll be moving it in/out of here, so we need an Option for safety.
    let mut renderer = Some(renderer);

    // Done loading - This is our main loop.
    // It just communicates events to the session and continuously ticks
    event_loop.run(move |event, _, flow| match event {
        Event::MainEventsCleared => {
            window.request_redraw();
        }
        Event::RedrawRequested(_) => {
            session.do_update();

            // Render
            let r = renderer.take().unwrap();
            match r.render(&session) {
                Ok(r) => {
                    renderer = Some(r);
                }
                Err(e) => {
                    println!("Error drawing: {}", full_error_display(e));

                    *flow = ControlFlow::Exit;
                }
            }
        }
        Event::WindowEvent {
            window_id: _,
            event: WindowEvent::Resized(_),
        } => {
            // (Attempt) resize
            let r = renderer.take().unwrap();
            match r.recreate_surface(&session) {
                Ok(r) => {
                    renderer = Some(r);
                }
                Err(e) => {
                    println!("Error resizing: {}", full_error_display(e));

                    *flow = ControlFlow::Exit;
                }
            }
        }
        _ => (),
    });
}
