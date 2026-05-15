#[derive(Clone, Copy, Default)]
pub struct ScrollAxis {
    pub offset:   f32,
    pub velocity: f32,
}

impl ScrollAxis {
    pub fn push(&mut self, delta: f32, max_speed: f32) {
        let new = self.velocity + delta;
        self.velocity = new.clamp(-max_speed, max_speed);
    }

    pub fn set_intent(&mut self, vel: f32) {
        self.velocity = vel;
    }

    pub fn tick(&mut self, friction: f32, max_offset: f32) -> bool {
        if self.velocity.abs() <= 0.1 {
            self.velocity = 0.0;
            return false;
        }

        let prev   = self.offset;
        self.offset = (self.offset + self.velocity).clamp(0.0, max_offset);

        if self.offset <= 0.0 || self.offset >= max_offset {
            self.velocity = 0.0;
        } else {
            self.velocity *= friction;
        }

        (self.offset - prev).abs() > 0.01
    }

    pub fn jump_to(&mut self, pos: f32, max_offset: f32) {
        self.offset   = pos.clamp(0.0, max_offset);
        self.velocity = 0.0;
    }

    pub fn reset(&mut self) {
        self.offset   = 0.0;
        self.velocity = 0.0;
    }
}
