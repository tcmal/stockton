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

//! The thing you play on and all the associated state.

use legion::systems::Builder;
use legion::*;

/// A loaded world.
pub struct Session {
    world: World,
    resources: Resources,
    schedule: Schedule,
}

impl Session {
    /// Create a new world from a level.
    /// The level can be any format, as long as it has the required features of a bsp.
    pub fn new<R: FnOnce(&mut Resources), S: FnOnce(&mut Builder)>(
        add_resources: R,
        add_systems: S,
    ) -> Session {
        let world = World::default();

        let mut resources = Resources::default();
        add_resources(&mut resources);

        let mut schedule = Schedule::builder();
        add_systems(&mut schedule);
        let schedule = schedule.build();

        Session {
            world,
            resources,
            schedule,
        }
    }

    pub fn do_update(&mut self) {
        self.schedule.execute(&mut self.world, &mut self.resources);
    }
}
