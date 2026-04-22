use quartz::Color;

// ── CursorStyle ───────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub enum CursorStyle { Line, Block, Underline }

impl CursorStyle {
    pub fn size(&self, line_height: f32, char_width: f32) -> (f32, f32, f32) {
        match self {
            CursorStyle::Line      => (2.0,        line_height, 0.0),
            CursorStyle::Block     => (char_width, line_height, 0.0),
            CursorStyle::Underline => (char_width, line_height, 0.0),
        }
    }

    pub fn build_image(&self, w: f32, h: f32) -> image::RgbaImage {
        let pw = w.ceil() as u32;
        let ph = h.ceil() as u32;
        let mut img = image::RgbaImage::new(pw.max(1), ph.max(1));
        match self {
            CursorStyle::Line => {
                for y in 0..ph { for x in 0..pw {
                    img.put_pixel(x, y, image::Rgba([220, 220, 220, 255]));
                }}
            }
            CursorStyle::Block => {
                let border  = 1u32;
                let fill    = image::Rgba([180, 140, 255,  60]);
                let outline = image::Rgba([200, 160, 255, 200]);
                for y in 0..ph { for x in 0..pw {
                    let on_border = x < border || y < border
                        || x >= pw.saturating_sub(border)
                        || y >= ph.saturating_sub(border);
                    img.put_pixel(x, y, if on_border { outline } else { fill });
                }}
            }
            CursorStyle::Underline => {
                let bar_h     = 3u32;
                let fill      = image::Rgba([180, 140, 255,  60]);
                let bar_color = image::Rgba([180, 140, 255, 200]);
                for y in 0..ph { for x in 0..pw {
                    img.put_pixel(x, y, if y >= ph.saturating_sub(bar_h) { bar_color } else { fill });
                }}
            }
        }
        img
    }
}

// ── Settings ──────────────────────────────────────────────────────────────────

#[derive(Clone, Copy)]
pub struct Settings {
    pub font_size:                f32,
    pub line_height_mul:          f32,
    pub char_width_mul:           f32,
    pub text_x:                   f32,
    pub text_y:                   f32,
    pub gutter_w:                 f32,
    pub backspace_deletes_before: bool,
    pub cursor_style:             CursorStyle,
    pub cursor_blink:             bool,
    pub auto_pairs:               bool,
    pub border_color:             (u8, u8, u8, u8),
    pub border_thickness:         f32,
    pub border_padding:           f32,
    pub scroll_accel:             f32,
    pub scroll_friction:          f32,
    pub scroll_max:               f32,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            font_size:                13.0,
            line_height_mul:          1.55,
            char_width_mul:           0.601,
            text_x:                   48.0,
            text_y:                   20.0,
            gutter_w:                 48.0,
            backspace_deletes_before: true,
            cursor_style:             CursorStyle::Underline,
            cursor_blink:             true,
            auto_pairs:               true,
            border_color:             (55, 55, 55, 255),
            border_thickness:         1.0,
            border_padding:           8.0,
            scroll_accel:             5.5,
            scroll_friction:          0.10,
            scroll_max:               90.0,
        }
    }
}

impl Settings {
    pub fn line_height(&self) -> f32 { self.font_size * self.line_height_mul }
    pub fn char_width(&self)  -> f32 { self.font_size * self.char_width_mul  }
    pub fn cursor_size(&self) -> (f32, f32, f32) {
        self.cursor_style.size(self.line_height(), self.char_width())
    }
}