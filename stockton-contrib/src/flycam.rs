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

use std::f32::consts::PI;

use stockton_input::{Axis, InputManager, Mouse};
use stockton_types::components::Transform;
use stockton_types::Vector3;

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
