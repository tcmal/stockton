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

use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Timing {
    pub delta_time: f32,

    pub(crate) last_frame_start: Instant,
}

impl Default for Timing {
    fn default() -> Self {
        Timing {
            delta_time: 0.0,

            last_frame_start: Instant::now(),
        }
    }
}

#[system]
pub fn update_deltatime(#[resource] timing: &mut Timing) {
    let now = Instant::now();
    timing.delta_time = now.duration_since(timing.last_frame_start).as_secs_f32();
    timing.last_frame_start = now;
}
