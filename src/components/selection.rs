// ============================================================================
//  components/selection.rs
//  Full text-selection subsystem — no Canvas dependency, pure logic.
// ============================================================================
//
//  COORDINATE CONVENTION
//  ─────────────────────
//  All (row, col) pairs use *character* (grapheme-cluster) indices, NOT byte
//  offsets.  Helper functions below convert to/from byte offsets when slicing
//  `&str`.  Callers that previously stored byte columns (e.g. sel_anchor,
//  sel_active in EditorState) should migrate to char cols; use
//  `byte_col_to_char_col` / `char_col_to_byte_col` for any IO boundary.
//
//  SELECTION RANGE ORDERING
//  ────────────────────────
//  SelectionRange is always (start, end) in document order (start <= end).
//  Zero-width selections (start == end) are represented as None by
//  `selection()`, so callers never have to special-case them.
//
// ============================================================================

// ----------------------------------------------------------------------------
//  Core types
// ----------------------------------------------------------------------------

/// A document-ordered, non-empty selection: `(start_row, start_char_col)` →
/// `(end_row, end_char_col)`.  Always guaranteed `start <= end`.
pub type SelectionRange = ((usize, usize), (usize, usize));

/// Anchor + active cursors, both in *character* column space.
/// The anchor is where the selection *started* (fixed); the active end tracks
/// the moving cursor.  Either may be None when no selection is live.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectionState {
    pub anchor: Option<(usize, usize)>,
    pub active: Option<(usize, usize)>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    /// Start a new selection anchored at `pos` (char coords).
    pub fn begin(&mut self, pos: (usize, usize)) {
        self.anchor = Some(pos);
        self.active = Some(pos);
    }

    /// Extend the active end to `pos`, keeping the anchor fixed.
    pub fn extend(&mut self, pos: (usize, usize)) {
        self.active = Some(pos);
    }

    /// Clear both ends (no selection).
    pub fn clear(&mut self) {
        self.anchor = None;
        self.active = None;
    }

    /// Returns the document-ordered range, or None for empty/unset selections.
    pub fn range(&self) -> Option<SelectionRange> {
        selection(self.anchor, self.active)
    }

    /// True when any non-zero-width selection is active.
    pub fn is_active(&self) -> bool {
        self.range().is_some()
    }

    /// Select an entire line (anchor at col 0, active at end of line).
    pub fn select_line(&mut self, row: usize, line: &str) {
        self.anchor = Some((row, 0));
        self.active = Some((row, char_len(line)));
    }

    /// Select the word under the cursor at `(row, char_col)`.
    /// Falls back to a single-character selection if no word boundary is found.
    pub fn select_word(&mut self, row: usize, char_col: usize, line: &str) {
        let (start, end) = word_bounds_at(line, char_col);
        self.anchor = Some((row, start));
        self.active = Some((row, end));
    }

    /// Select everything from (0, 0) to the last character of the last line.
    pub fn select_all(&mut self, lines: &[String]) {
        self.anchor = Some((0, 0));
        let last_row = lines.len().saturating_sub(1);
        let last_col = char_len(&lines[last_row]);
        self.active = Some((last_row, last_col));
    }
}

// ----------------------------------------------------------------------------
//  Core selection helpers
// ----------------------------------------------------------------------------

/// Returns the document-ordered `SelectionRange` from anchor/active, or `None`
/// when the selection is zero-width or either end is unset.
pub fn selection(
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
) -> Option<SelectionRange> {
    let a = anchor?;
    let b = active?;
    if a == b {
        return None;
    }
    if a <= b {
        Some((a, b))
    } else {
        Some((b, a))
    }
}

/// Extract the selected text from `lines` as a plain `String`.
/// Row/col values are *character* indices.
pub fn selected_text(
    lines: &[String],
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
) -> String {
    let Some(((r1, c1), (r2, c2))) = selection(anchor, active) else {
        return String::new();
    };
    if lines.is_empty() {
        return String::new();
    }

    // Clamp rows to valid range.
    let r1 = r1.min(lines.len().saturating_sub(1));
    let r2 = r2.min(lines.len().saturating_sub(1));

    if r1 == r2 {
        let line = &lines[r1];
        char_slice(line, c1, c2).to_string()
    } else {
        let first = &lines[r1];
        let mut out = char_slice(first, c1, char_len(first)).to_string();

        for row in (r1 + 1)..r2 {
            out.push('\n');
            out.push_str(&lines[row]);
        }

        let last = &lines[r2];
        out.push('\n');
        out.push_str(char_slice(last, 0, c2));
        out
    }
}

// ----------------------------------------------------------------------------
//  Click / drag positioning
// ----------------------------------------------------------------------------

/// Convert a pixel mouse position to a `(row, char_col)` document coordinate.
///
/// * `mouse_x / mouse_y` — virtual canvas position of the pointer.
/// * `code_x`            — left edge of the code area (after gutter).
/// * `text_top`          — top of the first visible row in canvas space.
/// * `h_scroll`          — current horizontal scroll offset (px).
/// * `slice_start`       — first document row currently rendered.
/// * `char_width`        — monospace character width (px).
/// * `line_height`       — height of one row (px).
/// * `lines`             — full document line vec.
///
/// The returned column is always clamped to `[0, char_len(line)]`.
pub fn mouse_to_doc_pos(
    mouse_x: f32,
    mouse_y: f32,
    code_x: f32,
    text_top: f32,
    h_scroll: f32,
    slice_start: usize,
    char_width: f32,
    line_height: f32,
    lines: &[String],
) -> (usize, usize) {
    // Row: how many line-heights below text_top is the pointer?
    let row_offset = ((mouse_y - text_top) / line_height).floor() as isize;
    let abs_row = (slice_start as isize + row_offset)
        .max(0)
        .min(lines.len().saturating_sub(1) as isize) as usize;

    // Column: horizontal offset relative to left edge of code area.
    let rel_x = mouse_x - code_x + h_scroll;
    // Add half a char width so clicks fall on the *nearest* column boundary.
    let char_col = ((rel_x + char_width * 0.5) / char_width).floor() as isize;
    let line = &lines[abs_row];
    let max_col = char_len(line) as isize;
    let char_col = char_col.max(0).min(max_col) as usize;

    (abs_row, char_col)
}

/// Given an existing selection `state` and a new mouse drag position, extend
/// the active end to the nearest document coordinate.  The anchor stays fixed.
pub fn drag_extend(
    state: &mut SelectionState,
    mouse_x: f32,
    mouse_y: f32,
    code_x: f32,
    text_top: f32,
    h_scroll: f32,
    slice_start: usize,
    char_width: f32,
    line_height: f32,
    lines: &[String],
) {
    let pos = mouse_to_doc_pos(
        mouse_x, mouse_y, code_x, text_top, h_scroll,
        slice_start, char_width, line_height, lines,
    );
    state.extend(pos);
}

// ----------------------------------------------------------------------------
//  Cursor movement helpers (arrow keys, home/end)
// ----------------------------------------------------------------------------

/// Move a cursor `(row, col)` left by one character, wrapping to the previous
/// line's end if at column 0.  Returns the new position.
pub fn move_left(row: usize, col: usize, lines: &[String]) -> (usize, usize) {
    if col > 0 {
        (row, col - 1)
    } else if row > 0 {
        let prev = row - 1;
        (prev, char_len(&lines[prev]))
    } else {
        (0, 0)
    }
}

/// Move a cursor right by one character, wrapping to the next line's start if
/// at the end of the current line.
pub fn move_right(row: usize, col: usize, lines: &[String]) -> (usize, usize) {
    let line_len = char_len(&lines[row]);
    if col < line_len {
        (row, col + 1)
    } else if row + 1 < lines.len() {
        (row + 1, 0)
    } else {
        (row, col)
    }
}

/// Jump to the start of the current line.
pub fn move_home(row: usize, line: &str) -> (usize, usize) {
    // Smart home: first non-whitespace, then column 0 on second press.
    let first_non_ws = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();
    (row, first_non_ws)
}

/// Jump to the end of the current line.
pub fn move_end(row: usize, line: &str) -> (usize, usize) {
    (row, char_len(line))
}

/// Move up by `count` rows, clamping to (0, 0).  Column is preserved but
/// clamped to the new line's length.
pub fn move_up(row: usize, col: usize, count: usize, lines: &[String]) -> (usize, usize) {
    let new_row = row.saturating_sub(count);
    let new_col = col.min(char_len(&lines[new_row]));
    (new_row, new_col)
}

/// Move down by `count` rows, clamping to the last line.
pub fn move_down(row: usize, col: usize, count: usize, lines: &[String]) -> (usize, usize) {
    let new_row = (row + count).min(lines.len().saturating_sub(1));
    let new_col = col.min(char_len(&lines[new_row]));
    (new_row, new_col)
}

/// Jump to `(0, 0)`.
pub fn move_doc_start() -> (usize, usize) {
    (0, 0)
}

/// Jump to the very end of the document.
pub fn move_doc_end(lines: &[String]) -> (usize, usize) {
    let last = lines.len().saturating_sub(1);
    (last, char_len(&lines[last]))
}

/// Move the cursor one word to the left (Ctrl+Left).
pub fn move_word_left(row: usize, col: usize, lines: &[String]) -> (usize, usize) {
    if col == 0 {
        if row == 0 {
            return (0, 0);
        }
        let prev = row - 1;
        return (prev, char_len(&lines[prev]));
    }
    let line = &lines[row];
    let new_col = prev_word_boundary(line, col);
    (row, new_col)
}

/// Move the cursor one word to the right (Ctrl+Right).
pub fn move_word_right(row: usize, col: usize, lines: &[String]) -> (usize, usize) {
    let line = &lines[row];
    let line_len = char_len(line);
    if col == line_len {
        if row + 1 < lines.len() {
            return (row + 1, 0);
        }
        return (row, col);
    }
    let new_col = next_word_boundary(line, col);
    (row, new_col)
}

// ----------------------------------------------------------------------------
//  Selection-aware deletion helpers
// ----------------------------------------------------------------------------

/// If a non-empty selection is active, returns the character range to delete:
/// `(start_row, start_col, end_row, end_col)` — all in char coords.
/// The caller should replace the range with an empty string and place the
/// cursor at `(start_row, start_col)`.
pub fn selection_delete_range(
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
) -> Option<((usize, usize), (usize, usize))> {
    selection(anchor, active)
}

// ----------------------------------------------------------------------------
//  Viewport overlay geometry
// ----------------------------------------------------------------------------

/// One highlighted row rectangle in canvas space.
#[derive(Debug, Clone, PartialEq)]
pub struct RowOverlay {
    /// Absolute document row index.
    pub abs_row: usize,
    /// Left edge in canvas (virtual screen) pixels.
    pub x_start: f32,
    /// Width in canvas pixels.  Always >= `min_width`.
    pub width: f32,
    /// Top edge in canvas pixels.
    pub y: f32,
}

/// Compute the set of highlight rectangles needed to draw the current
/// selection over the visible viewport.
///
/// Returns one `RowOverlay` per visible, selected document row.
/// Empty when there is no active selection or it falls entirely outside the
/// visible slice.
///
/// # Parameters
/// * `anchor` / `active`  — char-col selection endpoints.
/// * `lines`              — full document line vec.
/// * `slice_start`        — index of the first rendered document row.
/// * `visible_rows`       — how many rows are currently rendered.
/// * `text_top`           — canvas Y of the first rendered row.
/// * `code_x`             — canvas X of column 0 (left of code area).
/// * `h_scroll`           — horizontal scroll offset in pixels.
/// * `char_width`         — width of one character in pixels.
/// * `line_height`        — height of one row in pixels.
/// * `min_width`          — minimum overlay width (for empty-line selections).
#[allow(clippy::too_many_arguments)]
pub fn overlay_rects(
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
    lines: &[String],
    slice_start: usize,
    visible_rows: usize,
    text_top: f32,
    code_x: f32,
    h_scroll: f32,
    char_width: f32,
    line_height: f32,
    min_width: f32,
) -> Vec<RowOverlay> {
    let Some(((r1, c1), (r2, c2))) = selection(anchor, active) else {
        return vec![];
    };

    let total = lines.len();
    let mut out = Vec::with_capacity(visible_rows.min(r2 - r1 + 1));

    for i in 0..visible_rows {
        let abs_row = slice_start + i;

        // Skip rows outside the document or outside the selection.
        if abs_row >= total || abs_row < r1 || abs_row > r2 {
            continue;
        }

        let line = &lines[abs_row];
        let line_char_len = char_len(line);

        let start_col = if abs_row == r1 { c1.min(line_char_len) } else { 0 };
        let end_col = if abs_row == r2 {
            c2.min(line_char_len)
        } else {
            // For lines fully inside the selection, extend to end of line.
            // Add 1 to visually cover the newline character itself.
            line_char_len
        };

        let x_start = code_x + start_col as f32 * char_width - h_scroll;
        let x_end = code_x + end_col as f32 * char_width - h_scroll;
        // For fully-selected lines, show at least a half-char extra width
        // past the last character to indicate the newline is included.
        let extra = if abs_row < r2 && end_col == line_char_len {
            char_width * 0.5
        } else {
            0.0
        };
        let width = (x_end - x_start + extra).max(min_width);
        let y = text_top + i as f32 * line_height;

        out.push(RowOverlay {
            abs_row,
            x_start,
            width,
            y,
        });
    }

    out
}

// ----------------------------------------------------------------------------
//  UTF-8 / character-index utilities
// ----------------------------------------------------------------------------

/// Number of Unicode scalar values (chars) in `s`.
/// For ASCII source code this is identical to `s.len()`.
#[inline]
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

/// Slice `s` from character index `start` to `end` (exclusive), returning a
/// `&str` backed by the original string.
/// Clamps both indices to `[0, char_len(s)]`.
pub fn char_slice(s: &str, start: usize, end: usize) -> &str {
    let end = end.min(char_len(s));
    let start = start.min(end);
    let byte_start = char_col_to_byte_col(s, start);
    let byte_end = char_col_to_byte_col(s, end);
    &s[byte_start..byte_end]
}

/// Convert a *character* column index to a *byte* offset within `s`.
/// Clamps to `s.len()` if `char_col` exceeds the string length.
pub fn char_col_to_byte_col(s: &str, char_col: usize) -> usize {
    s.char_indices()
        .nth(char_col)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

/// Convert a *byte* offset to a *character* column index within `s`.
/// Panics (in debug) or saturates (in release) if `byte_col` is not on a
/// char boundary.
pub fn byte_col_to_char_col(s: &str, byte_col: usize) -> usize {
    s[..byte_col.min(s.len())].chars().count()
}

// ----------------------------------------------------------------------------
//  Word boundary helpers
// ----------------------------------------------------------------------------

/// Returns `(start, end)` character columns of the word containing `col`.
/// A "word" is a maximal run of alphanumeric/underscore characters, or a
/// single non-whitespace character if `col` is on punctuation/operator.
pub fn word_bounds_at(line: &str, col: usize) -> (usize, usize) {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    if len == 0 {
        return (0, 0);
    }
    let col = col.min(len.saturating_sub(1));

    let is_word_char = |c: char| c.is_alphanumeric() || c == '_';

    if is_word_char(chars[col]) {
        let start = (0..=col)
            .rev()
            .take_while(|&i| is_word_char(chars[i]))
            .last()
            .unwrap_or(col);
        let end = (col..len)
            .take_while(|&i| is_word_char(chars[i]))
            .last()
            .map(|i| i + 1)
            .unwrap_or(col + 1);
        (start, end)
    } else {
        // Punctuation / whitespace: select single character.
        (col, (col + 1).min(len))
    }
}

/// Previous word-start boundary from `col` (for Ctrl+Left).
fn prev_word_boundary(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut i = col.min(chars.len());

    // Step over any trailing whitespace.
    while i > 0 && chars[i - 1].is_whitespace() {
        i -= 1;
    }

    if i == 0 {
        return 0;
    }

    let is_word = chars[i - 1].is_alphanumeric() || chars[i - 1] == '_';

    if is_word {
        while i > 0 && (chars[i - 1].is_alphanumeric() || chars[i - 1] == '_') {
            i -= 1;
        }
    } else {
        // Step over a run of punctuation.
        while i > 0
            && !chars[i - 1].is_whitespace()
            && !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')
        {
            i -= 1;
        }
    }
    i
}

/// Next word-end boundary from `col` (for Ctrl+Right).
fn next_word_boundary(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = col;

    // Step over leading whitespace.
    while i < len && chars[i].is_whitespace() {
        i += 1;
    }

    if i == len {
        return len;
    }

    let is_word = chars[i].is_alphanumeric() || chars[i] == '_';

    if is_word {
        while i < len && (chars[i].is_alphanumeric() || chars[i] == '_') {
            i += 1;
        }
    } else {
        while i < len
            && !chars[i].is_whitespace()
            && !(chars[i].is_alphanumeric() || chars[i] == '_')
        {
            i += 1;
        }
    }
    i
}