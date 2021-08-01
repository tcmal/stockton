//! The thing you play on and all the associated state.

use legion::systems::Builder;
use legion::*;

/// A loaded world.
pub struct Session {
    pub world: World,
    pub resources: Resources,
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
