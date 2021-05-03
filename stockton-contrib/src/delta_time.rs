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
