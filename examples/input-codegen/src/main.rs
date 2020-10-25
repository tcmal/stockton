#[macro_use]
extern crate stockton_input_codegen;

use std::collections::BTreeMap;
use stockton_input::Action;
use stockton_input::{Axis, Button, InputManager, InputMutation};

#[derive(InputManager, Default, Debug, Clone)]
struct MovementInputs {
    #[axis]
    vertical: Axis,

    #[axis]
    horizontal: Axis,

    #[button]
    jump: Button,
}

const TEST_ACTIONS: [Action; 10] = [
    Action::KeyPress(1),
    Action::KeyRelease(1),
    Action::KeyPress(2),
    Action::KeyPress(3),
    Action::KeyRelease(2),
    Action::KeyRelease(3),
    Action::KeyPress(4),
    Action::KeyPress(5),
    Action::KeyRelease(4),
    Action::KeyRelease(5),
];

// For testing,   1 = w     2 = a
//                3 = s     4 = d
//                5 = jump
fn main() {
    let mut action_schema = BTreeMap::new();
    action_schema.insert(
        1,
        (MovementInputsFields::Vertical, InputMutation::PositiveAxis),
    );
    action_schema.insert(
        3,
        (MovementInputsFields::Vertical, InputMutation::NegativeAxis),
    );
    action_schema.insert(
        4,
        (
            MovementInputsFields::Horizontal,
            InputMutation::PositiveAxis,
        ),
    );
    action_schema.insert(
        2,
        (
            MovementInputsFields::Horizontal,
            InputMutation::NegativeAxis,
        ),
    );
    action_schema.insert(5, (MovementInputsFields::Jump, InputMutation::MapToButton));

    let mut manager = MovementInputsManager::new(action_schema);

    for action in TEST_ACTIONS.iter() {
        pretty_print_state(&manager.inputs);
        manager.handle_frame(std::iter::once(action));
    }
    pretty_print_state(&manager.inputs);
}

fn pretty_print_state(inputs: &MovementInputs) {
    if *inputs.vertical != 0 {
        print!("vertical = {}  ", *inputs.vertical);
    }
    if *inputs.horizontal != 0 {
        print!("horizontal = {}  ", *inputs.horizontal);
    }
    if inputs.jump.is_down() {
        if inputs.jump.is_hot {
            print!("jump!")
        } else {
            print!("jump")
        }
    }
    println!();
}
