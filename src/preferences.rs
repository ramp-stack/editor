use quartz::Color;

#[derive(Clone, Copy)]
pub enum CursorStyle {
    Line,
    Block,
    Underline,
}

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
                let outline = image::Rgba([220, 220, 220, 220]);
                let empty   = image::Rgba([0, 0, 0, 0]);
                for y in 0..ph { for x in 0..pw {
                    let on_border = x == 0 || y == 0
                        || x == pw.saturating_sub(1)
                        || y == ph.saturating_sub(1);
                    img.put_pixel(x, y, if on_border { outline } else { empty });
                }}
            }
            CursorStyle::Underline => {
                let bar_h     = 2u32;
                let bar_start = ph.saturating_sub(bar_h);
                let bar_color = image::Rgba([220, 220, 220, 255]);
                let empty     = image::Rgba([0, 0, 0, 0]);
                for y in 0..ph { for x in 0..pw {
                    img.put_pixel(x, y, if y >= bar_start { bar_color } else { empty });
                }}
            }
        }
        img
    }
}

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
            line_height_mul:          1.65,
            char_width_mul:           0.601,
            gutter_w:                 56.0,
            text_x:                   72.0,
            text_y:                   12.0,
            backspace_deletes_before: true,
            cursor_style:             CursorStyle::Underline,
            cursor_blink:             true,
            auto_pairs:               true,
            border_color:             (36, 40, 59, 255),
            border_thickness:         1.0,
            border_padding:           8.0,
            scroll_accel:             8.0,
            scroll_friction:          0.12,
            scroll_max:               120.0,
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