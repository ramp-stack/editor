pub mod viewer;
pub mod highlight;

use std::sync::{Arc, Mutex};
use quartz::{Font, Shared};

use crate::preferences::Settings;
use crate::editor::viewer::{FileMode, Lang, file_mode, file_lang};
use crate::editor::highlight::{ThemeColors, load_tm_theme};

pub use highlight::ThemeColors as EditorTheme;

// ── EditorState ───────────────────────────────────────────────────────────────

pub(crate) struct EditorState {
    pub lines:      Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub dirty:      bool,
    pub path:       String,
    pub mode:       FileMode,
    pub lang:       Lang,

    // Selection: anchor is where the drag/shift started, active is where it ends.
    // Both are (row, byte_col). None means no active selection.
    pub sel_anchor: Option<(usize, usize)>,
    pub sel_active: Option<(usize, usize)>,
}

impl EditorState {
    pub fn clamp_col(&mut self) {
        let max = self.lines[self.cursor_row].len();
        if self.cursor_col > max { self.cursor_col = max; }
    }

    pub fn save(&mut self) {
        if self.mode == FileMode::Image { return; }
        let content = self.lines.join("\n");
        match std::fs::write(&self.path, &content) {
            Ok(_)  => { println!("[autosave] wrote {} lines to {}", self.lines.len(), self.path); self.dirty = false; }
            Err(e) => println!("[autosave] error: {e}"),
        }
    }

    pub fn char_at(&self, row: usize, col: usize) -> Option<char> {
        self.lines.get(row)?.chars().nth(col)
    }

    pub fn char_before(&self, row: usize, col: usize) -> Option<char> {
        if col == 0 { return None; }
        self.lines.get(row)?.chars().nth(col - 1)
    }

    // Returns the selection as (start, end) in document order, or None.
    pub fn selection(&self) -> Option<((usize, usize), (usize, usize))> {
        let a = self.sel_anchor?;
        let b = self.sel_active?;
        if a == b { return None; } // zero-width selection = nothing
        if a <= b { Some((a, b)) } else { Some((b, a)) }
    }

    // Returns the selected text as a String.
    pub fn selected_text(&self) -> String {
        let Some(((r1, c1), (r2, c2))) = self.selection() else {
            return String::new();
        };
        if self.lines.is_empty() { return String::new(); }
        if r1 == r2 {
            let line = &self.lines[r1];
            let c1 = c1.min(line.len());
            let c2 = c2.min(line.len());
            line[c1..c2].to_string()
        } else {
            let first_line = &self.lines[r1];
            let c1 = c1.min(first_line.len());
            let mut out = first_line[c1..].to_string();
            for row in (r1 + 1)..r2 {
                out.push('\n');
                out.push_str(&self.lines[row]);
            }
            let last_line = &self.lines[r2];
            let c2 = c2.min(last_line.len());
            out.push('\n');
            out.push_str(&last_line[..c2]);
            out
        }
    }

    // Clear selection entirely.
    pub fn clear_selection(&mut self) {
        self.sel_anchor = None;
        self.sel_active = None;
    }

    // Set anchor to current cursor position (start of a new selection).
    pub fn anchor_at_cursor(&mut self) {
        self.sel_anchor = Some((self.cursor_row, self.cursor_col));
        self.sel_active = Some((self.cursor_row, self.cursor_col));
    }

    // Extend active end of selection to current cursor position.
    pub fn extend_selection_to_cursor(&mut self) {
        self.sel_active = Some((self.cursor_row, self.cursor_col));
    }
}

// ── ObjNames ──────────────────────────────────────────────────────────────────

pub(crate) struct ObjNames {
    pub bg:        String,
    pub gutter_bg: String,
    pub gutter:    String,
    pub code_text: String,
    pub cursor:    String,
}

impl ObjNames {
    pub fn from_prefix(p: &str) -> Self {
        Self {
            bg:        format!("{}_bg",        p),
            gutter_bg: format!("{}_gutter_bg", p),
            gutter:    format!("{}_gutter",    p),
            code_text: format!("{}_code_text", p),
            cursor:    format!("{}_cursor",    p),
        }
    }
}

// How many selection-highlight overlay objects to keep in the pool.
// Covers the tallest viewport we'd realistically see.
pub(crate) const SEL_OVERLAY_COUNT: usize = 80;

pub(crate) fn sel_overlay_name(prefix: &str, i: usize) -> String {
    format!("{}_sel_{}", prefix, i)
}

// ── Editor ────────────────────────────────────────────────────────────────────

pub struct Editor {
    pub(crate) id_prefix: Arc<String>,

    pub(crate) live_x: Shared<f32>,
    pub(crate) live_y: Shared<f32>,
    pub(crate) live_w: Shared<f32>,
    pub(crate) live_h: Shared<f32>,

    pub(crate) cfg:         Shared<Settings>,
    pub(crate) code_font:   Arc<Font>,
    pub(crate) gutter_font: Arc<Font>,
    pub(crate) theme:       Shared<ThemeColors>,

    pub(crate) state:          Arc<Mutex<EditorState>>,
    pub(crate) global_scroll:  Shared<f32>,
    pub(crate) h_scroll:       Shared<f32>,
    pub(crate) scroll_vel:     Shared<f32>,
    pub(crate) h_scroll_vel:   Shared<f32>,
    pub(crate) max_line_width: Shared<f32>,

    pub(crate) blink_timer:    Shared<f32>,
    pub(crate) idle_timer:     Shared<f32>,
    pub(crate) cursor_vis:     Shared<bool>,
    pub(crate) autosave_timer: Shared<f64>,
    pub(crate) img_loaded_key: Shared<String>,

    // True while the left mouse button is held for a drag-select.
    pub(crate) dragging: Shared<bool>,

    // Sustained scroll drive written by input handlers, consumed every tick.
    //
    // Unlike scroll_vel (which decays via friction), this value is injected
    // into scroll_vel on every tick while it is non-zero, overpowering friction
    // so the viewport keeps moving for as long as the key/button is held.
    // Cleared by on_mouse_release and when the cursor moves off the edge row.
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
        id: &str,
        x: f32, y: f32, w: f32, h: f32,
        code_font:   Arc<Font>,
        gutter_font: Arc<Font>,
        file_path:   &str,
        theme_bytes: &[u8],
        settings:    Settings,
    ) -> Self {
        let cfg   = Shared::new(settings);
        let theme = Shared::new(load_tm_theme(theme_bytes));
        let lines: Vec<String> = if file_mode(file_path) == FileMode::Image {
            vec![]
        } else {
            std::fs::read_to_string(file_path)
                .unwrap_or_else(|e| format!("// Failed to load {file_path}: {e}"))
                .lines().map(|l| l.to_string()).collect()
        };
        let lines = if lines.is_empty() && file_mode(file_path) == FileMode::Text {
            vec![String::new()]
        } else { lines };
        let state = Arc::new(Mutex::new(EditorState {
            lines, cursor_row: 0, cursor_col: 0, dirty: false,
            path: file_path.to_string(),
            mode: file_mode(file_path),
            lang: file_lang(file_path),
            sel_anchor: None,
            sel_active: None,
        }));
        Self {
            id_prefix: Arc::new(id.to_string()),
            live_x: Shared::new(x), live_y: Shared::new(y),
            live_w: Shared::new(w), live_h: Shared::new(h),
            cfg, code_font, gutter_font, theme, state,
            global_scroll:  Shared::new(0.0),
            h_scroll:       Shared::new(0.0),
            scroll_vel:     Shared::new(0.0),
            h_scroll_vel:   Shared::new(0.0),
            max_line_width: Shared::new(0.0),
            blink_timer:    Shared::new(0.0f32),
            idle_timer:     Shared::new(0.0f32),
            cursor_vis:     Shared::new(true),
            autosave_timer: Shared::new(0.0f64),
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
        if (old_w - w).abs() > 0.5 { *self.max_line_width.get_mut() = 0.0; }
    }

    pub fn get_bounds(&self) -> (f32, f32, f32, f32) {
        (*self.live_x.get(), *self.live_y.get(), *self.live_w.get(), *self.live_h.get())
    }

    pub fn open_file(&self, file_path: &str) {
        { let mut st = self.state.lock().unwrap(); if st.dirty { st.save(); } }
        let new_mode = file_mode(file_path);
        let new_lang = file_lang(file_path);
        let lines: Vec<String> = if new_mode == FileMode::Image { vec![] } else {
            let contents = std::fs::read_to_string(file_path)
                .unwrap_or_else(|e| format!("// Failed to load {file_path}: {e}"));
            if contents.is_empty() { vec![String::new()] }
            else { contents.lines().map(|l| l.to_string()).collect() }
        };
        {
            let mut st = self.state.lock().unwrap();
            st.lines      = lines;
            st.cursor_row = 0;
            st.cursor_col = 0;
            st.dirty      = false;
            st.path       = file_path.to_string();
            st.mode       = new_mode;
            st.lang       = new_lang;
            st.clear_selection();
        }
        *self.global_scroll.get_mut()  = 0.0;
        *self.h_scroll.get_mut()       = 0.0;
        *self.scroll_vel.get_mut()     = 0.0;
        *self.h_scroll_vel.get_mut()   = 0.0;
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
        *self.theme.get_mut()          = load_tm_theme(theme_bytes);
        *self.max_line_width.get_mut() = 0.0;
    }
}