pub mod state;

use quartz::{Color, Font, Shared};
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use crate::components::scroll::ScrollAxis;
use crate::components::theme::load_json_theme;
use crate::preferences::Settings;
use crate::editor::state::{load, EditorState};

pub struct ObjNames {
    pub bg:        String,
    pub gutter_bg: String,
    pub gutter:    String,
    pub code_text: String,
    pub cursor:    String,
}

impl ObjNames {
    pub fn from_prefix(p: &str) -> Self {
        Self {
            bg:        format!("{p}_bg"),
            gutter_bg: format!("{p}_gutter_bg"),
            gutter:    format!("{p}_gutter"),
            code_text: format!("{p}_code_text"),
            cursor:    format!("{p}_cursor"),
        }
    }
}

pub const SEL_OVERLAY_COUNT: usize = 80;

pub fn sel_overlay_name(prefix: &str, i: usize) -> String {
    format!("{prefix}_sel_{i}")
}

pub struct Editor {
    pub(crate) id_prefix: Arc<String>,
    pub(crate) live_x: Shared<f32>,
    pub(crate) live_y: Shared<f32>,
    pub(crate) live_w: Shared<f32>,
    pub(crate) live_h: Shared<f32>,
    pub(crate) cfg:   Shared<Settings>,
    pub(crate) code_font:   Arc<Font>,
    pub(crate) gutter_font: Arc<Font>,
    pub(crate) theme: Shared<HashMap<String, Color>>,
    pub(crate) state: Arc<Mutex<EditorState>>,
    pub(crate) v_scroll: Shared<ScrollAxis>,
    pub(crate) h_scroll: Shared<ScrollAxis>,
    pub(crate) max_line_width: Shared<f32>,
    pub(crate) blink_timer:    Shared<f32>,
    pub(crate) idle_timer:     Shared<f32>,
    pub(crate) cursor_vis:     Shared<bool>,
    pub(crate) autosave_timer: Shared<f64>,
    pub(crate) img_loaded_key: Shared<String>,
    pub(crate) dragging: Shared<bool>,
    pub(crate) scroll_intent: Shared<f32>,
}

impl Editor {
    pub fn new(
        x: f32, y: f32, w: f32, h: f32,
        code_font:   Arc<Font>,
        gutter_font: Arc<Font>,
        file_path:   &str,
        theme_bytes: &[u8],
        settings:    Settings,
    ) -> Self {
        Self::with_id("ed", x, y, w, h, code_font, gutter_font, file_path, theme_bytes, settings)
    }

    pub fn with_id(
        id:          &str,
        x: f32, y: f32, w: f32, h: f32,
        code_font:   Arc<Font>,
        gutter_font: Arc<Font>,
        file_path:   &str,
        theme_bytes: &[u8],
        settings:    Settings,
    ) -> Self {
        Self {
            id_prefix:      Arc::new(id.to_string()),
            live_x:         Shared::new(x),
            live_y:         Shared::new(y),
            live_w:         Shared::new(w),
            live_h:         Shared::new(h),
            cfg:            Shared::new(settings),
            code_font,
            gutter_font,
            theme:          Shared::new(load_json_theme(theme_bytes)),
            state:          Arc::new(Mutex::new(load(file_path))),
            v_scroll:       Shared::new(ScrollAxis::default()),
            h_scroll:       Shared::new(ScrollAxis::default()),
            max_line_width: Shared::new(0.0),
            blink_timer:    Shared::new(0.0),
            idle_timer:     Shared::new(0.0),
            cursor_vis:     Shared::new(true),
            autosave_timer: Shared::new(0.0),
            img_loaded_key: Shared::new(String::new()),
            dragging:       Shared::new(false),
            scroll_intent:  Shared::new(0.0),
        }
    }

    pub fn id(&self) -> &str { self.id_prefix.as_str() }

    pub fn set_bounds(&self, x: f32, y: f32, w: f32, h: f32) {
        let old_w = { *self.live_w.get() };
        *self.live_x.get_mut() = x;
        *self.live_y.get_mut() = y;
        *self.live_w.get_mut() = w;
        *self.live_h.get_mut() = h;
        if (old_w - w).abs() > 0.5 {
            *self.max_line_width.get_mut() = 0.0;
        }
    }

    pub fn get_bounds(&self) -> (f32, f32, f32, f32) {
        (*self.live_x.get(), *self.live_y.get(), *self.live_w.get(), *self.live_h.get())
    }

    pub fn open_file(&self, file_path: &str) {
        {
            let mut st = self.state.lock().unwrap();
            if st.dirty { st.save(); }
        }
        let new_state = load(file_path);
        {
            let mut st = self.state.lock().unwrap();
            *st = new_state;
        }
        self.v_scroll.get_mut().reset();
        self.h_scroll.get_mut().reset();
        *self.max_line_width.get_mut() = 0.0;
        *self.img_loaded_key.get_mut() = String::new();
        *self.dragging.get_mut()       = false;
        *self.scroll_intent.get_mut()  = 0.0;
    }

    pub fn apply_settings(&self, new_settings: Settings) {
        *self.cfg.get_mut()            = new_settings;
        *self.max_line_width.get_mut() = 0.0;
    }

    pub fn reload_theme(&self, theme_bytes: &[u8]) {
        *self.theme.get_mut()          = load_json_theme(theme_bytes);
        *self.max_line_width.get_mut() = 0.0;
    }

    pub fn mount(&self, cv: &mut flowmango::Canvas) {
        crate::objects::editor_obj::setup(cv, self);
    }

    pub fn register_callbacks(&self, cv: &mut flowmango::Canvas) {
        crate::logic::input::register(cv, self);
        crate::logic::editor_obj::register(cv, self);
    }
}
