// editor/state.rs — Core editor document + cursor state.

use crate::components::language::{file_lang, file_mode, FileMode, Lang};
use crate::components::selection;

pub struct EditorState {
    pub lines:      Vec<String>,
    pub cursor_row: usize,
    pub cursor_col: usize,
    pub dirty:      bool,
    pub path:       String,
    pub mode:       FileMode,
    pub lang:       Lang,

    // Selection anchor (where drag/shift started) and active end.
    // Both are (row, byte_col).  None = no selection.
    pub sel_anchor: Option<(usize, usize)>,
    pub sel_active: Option<(usize, usize)>,
}

impl EditorState {
    /// Clamp cursor_col to the length of the current line.
    pub fn clamp_col(&mut self) {
        let max = self.lines[self.cursor_row].len();
        if self.cursor_col > max {
            self.cursor_col = max;
        }
    }

    /// Write the document to disk if not in image mode.
    pub fn save(&mut self) {
        if self.mode == FileMode::Image { return; }
        let content = self.lines.join("\n");
        match std::fs::write(&self.path, &content) {
            Ok(_) => {
                println!("[autosave] wrote {} lines to {}", self.lines.len(), self.path);
                self.dirty = false;
            }
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

    // ── Selection helpers (delegate to components::selection) ─────────────────

    pub fn selection(&self) -> Option<((usize, usize), (usize, usize))> {
        selection::selection(self.sel_anchor, self.sel_active)
    }

    pub fn selected_text(&self) -> String {
        selection::selected_text(&self.lines, self.sel_anchor, self.sel_active)
    }

    pub fn clear_selection(&mut self) {
        self.sel_anchor = None;
        self.sel_active = None;
    }

    pub fn anchor_at_cursor(&mut self) {
        self.sel_anchor = Some((self.cursor_row, self.cursor_col));
        self.sel_active = Some((self.cursor_row, self.cursor_col));
    }

    pub fn extend_selection_to_cursor(&mut self) {
        self.sel_active = Some((self.cursor_row, self.cursor_col));
    }
}

/// Build `EditorState` by reading `file_path` from disk.
pub fn load(file_path: &str) -> EditorState {
    let mode = file_mode(file_path);
    let lang = file_lang(file_path);

    let lines: Vec<String> = if mode == FileMode::Image {
        vec![]
    } else {
        let contents = std::fs::read_to_string(file_path)
            .unwrap_or_else(|e| format!("// Failed to load {file_path}: {e}"));
        if contents.is_empty() {
            vec![String::new()]
        } else {
            contents.lines().map(|l| l.to_string()).collect()
        }
    };

    let lines = if lines.is_empty() && mode == FileMode::Text {
        vec![String::new()]
    } else {
        lines
    };

    EditorState {
        lines,
        cursor_row: 0,
        cursor_col: 0,
        dirty:      false,
        path:       file_path.to_string(),
        mode,
        lang,
        sel_anchor: None,
        sel_active: None,
    }
}
