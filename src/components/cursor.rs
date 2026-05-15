pub struct BlinkState {
    pub visible:    bool,
    idle_timer:     f32,
    blink_timer:    f32,
    idle_threshold: f32,
    blink_rate:     f32,
}

impl BlinkState {
    pub fn new(idle_threshold: f32, blink_rate: f32) -> Self {
        Self {
            visible:    true,
            idle_timer: 0.0,
            blink_timer: 0.0,
            idle_threshold,
            blink_rate,
        }
    }

    pub fn tick(&mut self, dt: f32, blinking_enabled: bool) -> bool {
        self.idle_timer  += dt;
        let should_blink  = blinking_enabled && self.idle_timer >= self.idle_threshold;

        if should_blink {
            self.blink_timer += dt;
            if self.blink_timer >= self.blink_rate {
                self.blink_timer = 0.0;
                self.visible     = !self.visible;
                return true;
            }
        } else {
            if !self.visible {
                self.visible = true;
                return true;
            }
        }
        false
    }

    pub fn reset(&mut self) {
        self.idle_timer  = 0.0;
        self.blink_timer = 0.0;
        self.visible     = true;
    }
}

pub fn cursor_position(
    cursor_row:  usize,
    cursor_col:  usize,
    slice_start: usize,
    text_top:    f32,
    code_x:      f32,
    h_scroll:    f32,
    char_width:  f32,
    line_height: f32,
) -> (f32, f32) {
    let x = code_x + cursor_col as f32 * char_width - h_scroll;
    let y = text_top + (cursor_row as f32 - slice_start as f32) * line_height;
    (x, y)
}

pub fn cursor_in_view(
    cursor_x: f32,
    cursor_y: f32,
    code_x:   f32,
    ex:       f32,
    ey:       f32,
    ew:       f32,
    eh:       f32,
    right_pad: f32,
    text_y:   f32,
) -> bool {
    cursor_x >= code_x
        && cursor_x < ex + ew - right_pad
        && cursor_y >= ey + text_y
        && cursor_y < ey + eh
}
