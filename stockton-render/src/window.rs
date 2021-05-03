use crate::Renderer;
use egui::Context;
use legion::systems::Runnable;
use log::debug;
use std::sync::Arc;
use stockton_levels::prelude::{MinBspFeatures, VulkanSystem};

use egui::{Output, PaintJobs, Pos2, RawInput, Ui};

use stockton_input::{Action as KBAction, InputManager, Mouse};

use winit::event::{
    ElementState, Event as WinitEvent, MouseButton, WindowEvent as WinitWindowEvent,
};
use winit::event_loop::ControlFlow;

#[derive(Debug, Clone, Copy)]
pub enum WindowEvent {
    SizeChanged(u32, u32),
    CloseRequested,
    KeyboardAction(KBAction),
    MouseAction(KBAction),
    MouseMoved(f32, f32),
    MouseLeft,
}

impl WindowEvent {
    pub fn from(winit_event: &WinitEvent<()>) -> Option<WindowEvent> {
        // TODO
        match winit_event {
            WinitEvent::WindowEvent { event, .. } => match event {
                WinitWindowEvent::CloseRequested => Some(WindowEvent::CloseRequested),
                WinitWindowEvent::Resized(size) => {
                    Some(WindowEvent::SizeChanged(size.width, size.height))
                }
                WinitWindowEvent::KeyboardInput { input, .. } => match input.state {
                    ElementState::Pressed => Some(WindowEvent::KeyboardAction(KBAction::KeyPress(
                        input.scancode,
                    ))),
                    ElementState::Released => Some(WindowEvent::KeyboardAction(
                        KBAction::KeyRelease(input.scancode),
                    )),
                },
                WinitWindowEvent::CursorMoved { position, .. } => Some(WindowEvent::MouseMoved(
                    position.x as f32,
                    position.y as f32,
                )),
                WinitWindowEvent::CursorLeft { .. } => Some(WindowEvent::MouseLeft),
                WinitWindowEvent::MouseInput { button, state, .. } => {
                    let mb: stockton_input::MouseButton = match button {
                        MouseButton::Left => stockton_input::MouseButton::Left,
                        MouseButton::Right => stockton_input::MouseButton::Right,
                        MouseButton::Middle => stockton_input::MouseButton::Middle,
                        MouseButton::Other(x) => stockton_input::MouseButton::Other(*x),
                    };

                    match state {
                        ElementState::Pressed => {
                            Some(WindowEvent::MouseAction(KBAction::MousePress(mb)))
                        }
                        ElementState::Released => {
                            Some(WindowEvent::MouseAction(KBAction::MouseRelease(mb)))
                        }
                    }
                }
                _ => None,
            },
            _ => None,
        }
    }
}

pub struct UiState {
    pub(crate) ctx: Arc<Context>,
    pub(crate) raw_input: RawInput,
    ui: Option<Ui>,

    pub(crate) last_tex_ver: u64,
}

impl UiState {
    pub fn ui(&mut self) -> &mut Ui {
        if self.ui.is_none() {
            self.ui = Some(self.begin_frame());
        }
        self.ui.as_mut().unwrap()
    }
    fn begin_frame(&mut self) -> Ui {
        self.ctx.begin_frame(self.raw_input.take())
    }

    pub fn end_frame(&mut self) -> (Output, PaintJobs) {
        self.ui = None;
        self.ctx.end_frame()
    }

    fn set_mouse_pos(&mut self, x: f32, y: f32) {
        self.raw_input.mouse_pos = Some(Pos2 { x, y })
    }

    fn set_mouse_left(&mut self) {
        self.raw_input.mouse_pos = None;
    }
    fn set_dimensions(&mut self, w: u32, h: u32) {
        self.raw_input.screen_size = egui::math::Vec2 {
            x: w as f32,
            y: h as f32,
        }
    }
    fn set_pixels_per_point(&mut self, ppp: Option<f32>) {
        self.raw_input.pixels_per_point = ppp;
    }

    pub fn dimensions(&self) -> egui::math::Vec2 {
        self.raw_input.screen_size
    }

    fn handle_action(&mut self, action: KBAction) {
        // TODO
        match action {
            KBAction::MousePress(stockton_input::MouseButton::Left) => {
                self.raw_input.mouse_down = true;
            }
            KBAction::MouseRelease(stockton_input::MouseButton::Right) => {
                self.raw_input.mouse_down = false;
            }
            _ => (),
        }
    }

    pub fn new<T: MinBspFeatures<VulkanSystem>>(renderer: &Renderer<T>) -> Self {
        let mut state = UiState {
            ctx: Context::new(),
            raw_input: RawInput::default(),
            ui: None,
            last_tex_ver: 0,
        };

        let props = &renderer.context.target_chain.properties;
        state.set_dimensions(props.extent.width, props.extent.height);
        state.set_pixels_per_point(Some(renderer.context.pixels_per_point));
        debug!("{:?}", state.raw_input);
        state
    }
}

#[system]
/// A system to process the window events sent to renderer by the winit event loop.
pub fn _process_window_events<
    T: 'static + InputManager,
    M: 'static + MinBspFeatures<VulkanSystem>,
>(
    #[resource] renderer: &mut Renderer<'static, M>,
    #[resource] manager: &mut T,
    #[resource] mouse: &mut Mouse,
    #[resource] ui_state: &mut UiState,
    #[state] actions_buf: &mut Vec<KBAction>,
) {
    let mut actions_buf_cursor = 0;
    let mut mouse_delta = mouse.abs;

    while let Ok(event) = renderer.window_events.try_recv() {
        match event {
            WindowEvent::SizeChanged(w, h) => {
                renderer.resize();
                ui_state.set_dimensions(w, h);
            }
            WindowEvent::CloseRequested => {
                let mut flow = renderer.update_control_flow.write().unwrap();
                // TODO: Let everything know this is our last frame
                *flow = ControlFlow::Exit;
            }
            WindowEvent::KeyboardAction(action) => {
                if actions_buf_cursor >= actions_buf.len() {
                    actions_buf.push(action);
                } else {
                    actions_buf[actions_buf_cursor] = action;
                }
                actions_buf_cursor += 1;

                ui_state.handle_action(action);
            }
            WindowEvent::MouseMoved(x, y) => {
                mouse_delta.x = x;
                mouse_delta.y = y;

                ui_state.set_mouse_pos(x, y);
            }
            WindowEvent::MouseLeft => {
                ui_state.set_mouse_left();
            }
            WindowEvent::MouseAction(action) => {
                if actions_buf_cursor >= actions_buf.len() {
                    actions_buf.push(action);
                } else {
                    actions_buf[actions_buf_cursor] = action;
                }
                actions_buf_cursor += 1;

                ui_state.handle_action(action);
            }
        };
    }

    mouse.handle_frame(mouse_delta);

    manager.handle_frame(&actions_buf[0..actions_buf_cursor]);
}

pub fn process_window_events_system<
    T: 'static + InputManager,
    M: 'static + MinBspFeatures<VulkanSystem>,
>() -> impl Runnable {
    _process_window_events_system::<T, M>(Vec::with_capacity(4))
}
