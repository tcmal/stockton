use std::f32::consts::PI;

use stockton_input::{Axis, InputManager, Mouse};
use stockton_skeleton::{components::Transform, types::Vector3};

use crate::delta_time::Timing;

pub trait FlycamInput {
    fn get_x_axis(&self) -> &Axis;
    fn get_y_axis(&self) -> &Axis;
    fn get_z_axis(&self) -> &Axis;
}

pub struct FlycamControlled {
    pub speed: f32,
    pub sensitivity: f32,
}

impl FlycamControlled {
    pub fn new(speed: f32, pixels_per_360: f32) -> Self {
        FlycamControlled {
            speed,
            sensitivity: (2.0 * PI) / pixels_per_360,
        }
    }
}

#[system(for_each)]
pub fn flycam_move<T>(
    #[resource] manager: &T,
    #[resource] timing: &Timing,
    #[resource] mouse: &Mouse,
    transform: &mut Transform,
    flycam: &FlycamControlled,
) where
    T: 'static + InputManager,
    T::Inputs: FlycamInput,
{
    let inputs = manager.get_inputs();
    let delta = Vector3::new(
        **inputs.get_x_axis() as f32 * flycam.speed * timing.delta_time,
        **inputs.get_y_axis() as f32 * flycam.speed * timing.delta_time,
        **inputs.get_z_axis() as f32 * flycam.speed * timing.delta_time,
    );

    transform.translate(delta);

    let rotation = mouse.delta * flycam.sensitivity;
    transform.rotate(Vector3::new(-rotation.y, rotation.x, 0.0));
}
