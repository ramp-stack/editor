use crate::components::language::{file_lang, file_mode, FileMode, Lang};
use crate::components::selection::{self, SelectionRange, SelectionState};
use std::sync::atomic::{AtomicU64, Ordering};

// Global counter — each file load gets a unique version so the render loop
// always detects a change, even when two files are opened without any edits.
static NEXT_VERSION: AtomicU64 = AtomicU64::new(1);

fn next_version() -> u64 {
    NEXT_VERSION.fetch_add(1, Ordering::Relaxed)
}

pub struct EditorState {
    pub lines:           Vec<String>,
    pub cursor_row:      usize,
    pub cursor_col:      usize,
    pub dirty:           bool,
    pub path:            String,
    pub mode:            FileMode,
    pub lang:            Lang,
    pub sel:             SelectionState,
    pub content_version: u64,
}

impl EditorState {
    pub fn clamp_col(&mut self) {
        let max = selection::char_len(&self.lines[self.cursor_row]);
        if self.cursor_col > max { self.cursor_col = max; }
    }

    pub fn bump(&mut self) {
        self.content_version = next_version();
        self.dirty = true;
    }

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

    pub fn selection(&self) -> Option<SelectionRange> {
        self.sel.range()
    }

    pub fn selected_text(&self) -> String {
        selection::selected_text(&self.lines, self.sel.anchor, self.sel.active)
    }

    pub fn clear_selection(&mut self)          { self.sel.clear(); }
    pub fn anchor_at_cursor(&mut self)         { self.sel.begin((self.cursor_row, self.cursor_col)); }
    pub fn extend_selection_to_cursor(&mut self) { self.sel.extend((self.cursor_row, self.cursor_col)); }
    pub fn select_all(&mut self)               { self.sel.select_all(&self.lines); }

    pub fn select_word_at_cursor(&mut self) {
        let row  = self.cursor_row;
        let col  = self.cursor_col;
        let line = self.lines[row].clone();
        self.sel.select_word(row, col, &line);
    }
}

pub fn load(file_path: &str) -> EditorState {
    let mode = file_mode(file_path);
    let lang = file_lang(file_path);

    let lines: Vec<String> = if mode == FileMode::Image {
        vec![]
    } else {
        let contents = std::fs::read_to_string(file_path)
            .unwrap_or_else(|e| format!("// Failed to load {file_path}: {e}"));
        if contents.is_empty() { vec![String::new()] }
        else { contents.lines().map(|l| l.to_string()).collect() }
    };

    let lines = if lines.is_empty() && mode == FileMode::Text {
        vec![String::new()]
    } else {
        lines
    };

    EditorState {
        lines,
        cursor_row:      0,
        cursor_col:      0,
        dirty:           false,
        path:            file_path.to_string(),
        mode,
        lang,
        sel:             SelectionState::new(),
        // Always unique — never 0, never collides with a previous file's version.
        content_version: next_version(),
    }
}