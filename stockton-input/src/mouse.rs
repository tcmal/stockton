use stockton_types::Vector2;

#[derive(Debug, Clone)]
pub struct Mouse {
    pub abs: Vector2,
    pub delta: Vector2,
}

impl Default for Mouse {
    fn default() -> Self {
        Mouse {
            abs: Vector2::zeros(),
            delta: Vector2::zeros(),
        }
    }
}

impl Mouse {
    pub fn handle_frame(&mut self, new: Vector2) {
        self.delta = new - self.abs;
        self.abs = new;
    }
}
