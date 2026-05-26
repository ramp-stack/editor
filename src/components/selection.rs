pub type SelectionRange = ((usize, usize), (usize, usize));

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct SelectionState {
    pub anchor: Option<(usize, usize)>,
    pub active: Option<(usize, usize)>,
}

impl SelectionState {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn begin(&mut self, pos: (usize, usize)) {
        self.anchor = Some(pos);
        self.active = Some(pos);
    }

    pub fn extend(&mut self, pos: (usize, usize)) {
        self.active = Some(pos);
    }
    
    pub fn clear(&mut self) {
        self.anchor = None;
        self.active = None;
    }

    pub fn range(&self) -> Option<SelectionRange> {
        selection(self.anchor, self.active)
    }

    pub fn is_active(&self) -> bool {
        self.range().is_some()
    }

    pub fn select_line(&mut self, row: usize, line: &str) {
        self.anchor = Some((row, 0));
        self.active = Some((row, char_len(line)));
    }

    pub fn select_word(&mut self, row: usize, char_col: usize, line: &str) {
        let (start, end) = word_bounds_at(line, char_col);
        self.anchor = Some((row, start));
        self.active = Some((row, end));
    }

    pub fn select_all(&mut self, lines: &[String]) {
        self.anchor = Some((0, 0));
        let last_row = lines.len().saturating_sub(1);
        let last_col = char_len(&lines[last_row]);
        self.active = Some((last_row, last_col));
    }
}

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
    let row_offset = ((mouse_y - text_top) / line_height).floor() as isize;
    let abs_row = (slice_start as isize + row_offset)
        .max(0)
        .min(lines.len().saturating_sub(1) as isize) as usize;

    let rel_x = mouse_x - code_x + h_scroll;
    let char_col = ((rel_x + char_width * 0.5) / char_width).floor() as isize;
    let line = &lines[abs_row];
    let max_col = char_len(line) as isize;
    let char_col = char_col.max(0).min(max_col) as usize;

    (abs_row, char_col)
}

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

pub fn move_home(row: usize, line: &str) -> (usize, usize) {
    let first_non_ws = line
        .chars()
        .take_while(|c| c.is_whitespace())
        .count();
    (row, first_non_ws)
}

pub fn move_end(row: usize, line: &str) -> (usize, usize) {
    (row, char_len(line))
}

pub fn move_up(row: usize, col: usize, count: usize, lines: &[String]) -> (usize, usize) {
    let new_row = row.saturating_sub(count);
    let new_col = col.min(char_len(&lines[new_row]));
    (new_row, new_col)
}

pub fn move_down(row: usize, col: usize, count: usize, lines: &[String]) -> (usize, usize) {
    let new_row = (row + count).min(lines.len().saturating_sub(1));
    let new_col = col.min(char_len(&lines[new_row]));
    (new_row, new_col)
}

pub fn move_doc_start() -> (usize, usize) {
    (0, 0)
}

pub fn move_doc_end(lines: &[String]) -> (usize, usize) {
    let last = lines.len().saturating_sub(1);
    (last, char_len(&lines[last]))
}

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

pub fn selection_delete_range(
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
) -> Option<((usize, usize), (usize, usize))> {
    selection(anchor, active)
}

#[derive(Debug, Clone, PartialEq)]
pub struct RowOverlay {
    pub abs_row: usize,
    pub x_start: f32,
    pub width: f32,
    pub y: f32,
}


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

        if abs_row >= total || abs_row < r1 || abs_row > r2 {
            continue;
        }

        let line = &lines[abs_row];
        let line_char_len = char_len(line);

        let start_col = if abs_row == r1 { c1.min(line_char_len) } else { 0 };
        let end_col = if abs_row == r2 {
            c2.min(line_char_len)
        } else {
            line_char_len
        };

        let x_start = code_x + start_col as f32 * char_width - h_scroll;
        let x_end = code_x + end_col as f32 * char_width - h_scroll;
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

#[inline]
pub fn char_len(s: &str) -> usize {
    s.chars().count()
}

pub fn char_slice(s: &str, start: usize, end: usize) -> &str {
    let end = end.min(char_len(s));
    let start = start.min(end);
    let byte_start = char_col_to_byte_col(s, start);
    let byte_end = char_col_to_byte_col(s, end);
    &s[byte_start..byte_end]
}

pub fn char_col_to_byte_col(s: &str, char_col: usize) -> usize {
    s.char_indices()
        .nth(char_col)
        .map(|(b, _)| b)
        .unwrap_or(s.len())
}

pub fn byte_col_to_char_col(s: &str, byte_col: usize) -> usize {
    s[..byte_col.min(s.len())].chars().count()
}

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
        (col, (col + 1).min(len))
    }
}

fn prev_word_boundary(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let mut i = col.min(chars.len());

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
        while i > 0
            && !chars[i - 1].is_whitespace()
            && !(chars[i - 1].is_alphanumeric() || chars[i - 1] == '_')
        {
            i -= 1;
        }
    }
    i
}

fn next_word_boundary(line: &str, col: usize) -> usize {
    let chars: Vec<char> = line.chars().collect();
    let len = chars.len();
    let mut i = col;

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