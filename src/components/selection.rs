pub type SelectionRange = ((usize, usize), (usize, usize));

pub fn selection(
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
) -> Option<SelectionRange> {
    let a = anchor?;
    let b = active?;
    if a == b { return None; }
    if a <= b { Some((a, b)) } else { Some((b, a)) }
}

pub fn selected_text(
    lines:  &[String],
    anchor: Option<(usize, usize)>,
    active: Option<(usize, usize)>,
) -> String {
    let Some(((r1, c1), (r2, c2))) = selection(anchor, active) else {
        return String::new();
    };
    if lines.is_empty() { return String::new(); }

    if r1 == r2 {
        let line = &lines[r1];
        let c1   = c1.min(line.len());
        let c2   = c2.min(line.len());
        line[c1..c2].to_string()
    } else {
        let first = &lines[r1];
        let c1    = c1.min(first.len());
        let mut out = first[c1..].to_string();
        for row in (r1 + 1)..r2 {
            out.push('\n');
            out.push_str(&lines[row]);
        }
        let last = &lines[r2];
        let c2   = c2.min(last.len());
        out.push('\n');
        out.push_str(&last[..c2]);
        out
    }
}

pub struct RowOverlay {
    pub abs_row: usize,
    pub x_start: f32,
    pub width:   f32,
    pub y:       f32,
}

#[allow(clippy::too_many_arguments)]
pub fn overlay_rects(
    anchor:       Option<(usize, usize)>,
    active:       Option<(usize, usize)>,
    lines:        &[String],
    slice_start:  usize,
    visible_rows: usize,
    text_top:     f32,
    code_x:       f32,
    h_scroll:     f32,
    char_width:   f32,
    line_height:  f32,
    min_width:    f32,
) -> Vec<RowOverlay> {
    let Some(((r1, c1), (r2, c2))) = selection(anchor, active) else {
        return vec![];
    };

    let total = lines.len();
    let mut out = Vec::new();

    for i in 0..visible_rows {
        let abs_row = slice_start + i;
        if abs_row >= total || abs_row < r1 || abs_row > r2 { continue; }

        let line      = &lines[abs_row];
        let start_col = if abs_row == r1 { c1.min(line.len()) } else { 0 };
        let end_col   = if abs_row == r2 { c2.min(line.len()) } else { line.len() };

        let x_start = code_x + start_col as f32 * char_width - h_scroll;
        let x_end   = code_x + end_col   as f32 * char_width - h_scroll;
        let width   = (x_end - x_start).max(min_width);
        let y       = text_top + i as f32 * line_height;

        out.push(RowOverlay { abs_row, x_start, width, y });
    }
    out
}
