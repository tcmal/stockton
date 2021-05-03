/// A thing that pressing a button can do to an input.
#[derive(Debug, Clone, Copy)]
pub enum InputMutation {
    MapToButton,
    NegativeAxis,
    PositiveAxis,
}

#[derive(Debug, Clone, Copy)]
pub enum MouseButton {
    Left,
    Right,
    Middle,
    Other(u8),
}

impl MouseButton {
    fn keycode(&self) -> u32 {
        u32::MAX
            - match self {
                MouseButton::Left => 0,
                MouseButton::Right => 1,
                MouseButton::Middle => 2,
                MouseButton::Other(x) => *x as u32,
            }
    }
}

/// A key being pressed or released
#[derive(Debug, Clone, Copy)]
pub enum Action {
    KeyPress(u32),
    KeyRelease(u32),
    MousePress(MouseButton),
    MouseRelease(MouseButton),
}

impl Action {
    pub fn keycode(&self) -> u32 {
        match self {
            Action::KeyPress(x) => *x,
            Action::KeyRelease(x) => *x,
            Action::MousePress(x) => x.keycode(),
            Action::MouseRelease(x) => x.keycode(),
        }
    }
    pub fn is_down(&self) -> bool {
        match self {
            Action::KeyPress(_) => true,
            Action::MousePress(_) => true,
            Action::KeyRelease(_) => false,
            Action::MouseRelease(_) => false,
        }
    }
}

pub trait InputManager {
    type Inputs;

    fn handle_frame<'a, X: IntoIterator<Item = &'a Action>>(&mut self, actions: X);
    fn get_inputs(&self) -> &Self::Inputs;
}
