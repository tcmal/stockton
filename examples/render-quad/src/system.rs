//! An example system that just alternates the colours of our triangle

use stockton_skeleton::types::Vector3;

/// RGB Channels
#[derive(Debug, Clone, Copy)]
enum ColorChannel {
    Red,
    Green,
    Blue,
}

/// A component for our entity.
#[derive(Debug, Clone, Copy)]
pub struct ExampleState {
    channel: ColorChannel,
    falling: bool,
    col_val: Vector3,
}

impl ExampleState {
    pub fn color(&self) -> Vector3 {
        self.col_val
    }
}

impl Default for ExampleState {
    fn default() -> Self {
        Self {
            channel: ColorChannel::Red,
            falling: true,
            col_val: Vector3::new(1.0, 1.0, 1.0),
        }
    }
}

/// The speed at which we change colour
const TRANSITION_SPEED: f32 = 0.1;

/// Keep changing the colour of any ExampleStates in our world.
#[system(for_each)]
pub fn mutate_state(state: &mut ExampleState) {
    // Which value we're changing
    let val = match state.channel {
        ColorChannel::Red => &mut state.col_val.x,
        ColorChannel::Green => &mut state.col_val.y,
        ColorChannel::Blue => &mut state.col_val.z,
    };

    if state.falling {
        *val -= TRANSITION_SPEED;

        // Fall, then rise
        if *val <= 0.0 {
            *val = 0.0;
            state.falling = false;
        }
    } else {
        *val += TRANSITION_SPEED;

        if *val >= 1.0 {
            *val = 1.0;

            // Rather than going back to falling, go to the next channel
            state.falling = true;
            state.channel = match state.channel {
                ColorChannel::Red => ColorChannel::Green,
                ColorChannel::Green => ColorChannel::Blue,
                ColorChannel::Blue => ColorChannel::Red,
            }
        }
    }
}
