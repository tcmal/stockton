use stockton_input::{Action as KBAction, InputManager, Mouse};
use stockton_skeleton::{
    draw_passes::{DrawPass, Singular},
    Renderer,
};

use std::sync::{
    mpsc::{channel, Receiver, Sender},
    Arc, RwLock,
};

use egui::{CtxRef, Event, Modifiers, Output, Pos2, RawInput, Rect, Vec2};
use epaint::ClippedShape;
use legion::systems::Runnable;
use log::debug;
use winit::{
    event::{ElementState, Event as WinitEvent, MouseButton, WindowEvent as WinitWindowEvent},
    event_loop::ControlFlow,
};

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
    ctx: CtxRef,
    raw_input: RawInput,
    frame_active: bool,

    modifiers: Modifiers,
    pointer_pos: Pos2,
}

impl UiState {
    pub fn populate_initial_state<T: DrawPass<Singular>>(&mut self, renderer: &Renderer<T>) {
        let props = renderer.context().properties();
        self.set_dimensions(props.extent.width, props.extent.height);
        self.set_pixels_per_point(Some(renderer.context().pixels_per_point()));
        debug!("{:?}", self.raw_input);
    }

    #[inline]
    pub fn ctx(&mut self) -> &CtxRef {
        if !self.frame_active {
            self.begin_frame()
        }
        &self.ctx
    }

    #[inline]
    fn begin_frame(&mut self) {
        #[allow(deprecated)]
        let new_raw_input = RawInput {
            scroll_delta: Vec2::new(0.0, 0.0),
            zoom_delta: 0.0,
            screen_size: self.raw_input.screen_size,
            screen_rect: self.raw_input.screen_rect,
            pixels_per_point: self.raw_input.pixels_per_point,
            time: self.raw_input.time,
            predicted_dt: self.raw_input.predicted_dt,
            modifiers: self.modifiers,
            events: Vec::new(),
        };
        self.ctx.begin_frame(self.raw_input.take());
        self.raw_input = new_raw_input;
        self.frame_active = true;
    }

    #[inline]
    pub(crate) fn end_frame(&mut self) -> (Output, Vec<ClippedShape>) {
        self.frame_active = false;
        self.ctx.end_frame()
    }

    #[inline]
    pub fn dimensions(&self) -> Option<egui::math::Vec2> {
        Some(self.raw_input.screen_rect?.size())
    }

    fn set_mouse_pos(&mut self, x: f32, y: f32) {
        self.raw_input
            .events
            .push(Event::PointerMoved(Pos2::new(x, y)));

        self.pointer_pos = Pos2::new(x, y);
    }

    fn set_mouse_left(&mut self) {
        self.raw_input.events.push(Event::PointerGone);
    }

    fn set_dimensions(&mut self, w: u32, h: u32) {
        self.raw_input.screen_rect =
            Some(Rect::from_x_y_ranges(0.0..=(w as f32), 0.0..=(h as f32)));
    }
    fn set_pixels_per_point(&mut self, ppp: Option<f32>) {
        debug!("Using {:?} pixels per point", ppp);
        self.raw_input.pixels_per_point = ppp;
    }

    fn handle_action(&mut self, action: KBAction) {
        // TODO
        match action {
            KBAction::MousePress(btn) => {
                self.raw_input.events.push(Event::PointerButton {
                    pos: self.pointer_pos,
                    button: match btn {
                        stockton_input::MouseButton::Left => egui::PointerButton::Primary,
                        stockton_input::MouseButton::Right => egui::PointerButton::Secondary,
                        stockton_input::MouseButton::Middle => egui::PointerButton::Middle,
                        stockton_input::MouseButton::Other(_) => todo!(),
                    },
                    pressed: true,
                    modifiers: self.modifiers,
                });
            }
            KBAction::MouseRelease(btn) => {
                self.raw_input.events.push(Event::PointerButton {
                    pos: self.pointer_pos,
                    button: match btn {
                        stockton_input::MouseButton::Left => egui::PointerButton::Primary,
                        stockton_input::MouseButton::Right => egui::PointerButton::Secondary,
                        stockton_input::MouseButton::Middle => egui::PointerButton::Middle,
                        stockton_input::MouseButton::Other(_) => todo!(),
                    },
                    pressed: false,
                    modifiers: self.modifiers,
                });
            }
            _ => (),
        }
    }
}

impl Default for UiState {
    fn default() -> Self {
        UiState {
            ctx: CtxRef::default(),
            raw_input: RawInput::default(),
            frame_active: false,
            modifiers: Default::default(),
            pointer_pos: Pos2::new(0.0, 0.0),
        }
    }
}

pub struct WindowFlow {
    window_events: Receiver<WindowEvent>,
    update_control_flow: Arc<RwLock<ControlFlow>>,
}

impl WindowFlow {
    pub fn new(update_control_flow: Arc<RwLock<ControlFlow>>) -> (Self, Sender<WindowEvent>) {
        let (tx, rx) = channel();
        (
            Self {
                window_events: rx,
                update_control_flow,
            },
            tx,
        )
    }
}

#[system]
/// A system to process the window events sent to renderer by the winit event loop.
pub fn _process_window_events<T: 'static + InputManager, DP: 'static + DrawPass<Singular>>(
    #[resource] window_channel: &mut WindowFlow,
    #[resource] manager: &mut T,
    #[resource] mouse: &mut Mouse,
    #[resource] ui_state: &mut UiState,
    #[state] actions_buf: &mut Vec<KBAction>,
) {
    let mut actions_buf_cursor = 0;
    let mut mouse_delta = mouse.abs;

    while let Ok(event) = window_channel.window_events.try_recv() {
        match event {
            WindowEvent::SizeChanged(w, h) => {
                ui_state.set_dimensions(w, h);
            }
            WindowEvent::CloseRequested => {
                let mut flow = window_channel.update_control_flow.write().unwrap();
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

pub fn process_window_events_system<T: 'static + InputManager, DP: 'static + DrawPass<Singular>>(
) -> impl Runnable {
    _process_window_events_system::<T, DP>(Vec::with_capacity(4))
}
