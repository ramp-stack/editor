use quartz::{NamedKey, Key, MouseButton};
use flowmango::Canvas;

use crate::constants::RIGHT_PAD;
use crate::editor::Editor;

fn auto_pair_close(c: char) -> Option<char> {
    match c { '(' => Some(')'), '[' => Some(']'), '{' => Some('}'), '"' => Some('"'), _ => None }
}

fn is_auto_pair(open: char, close: char) -> bool {
    matches!((open, close), ('(', ')') | ('[', ']') | ('{', '}') | ('"', '"'))
}

// Convert a mouse X position into a byte column for `line`.
fn mouse_col(mx: f32, ex: f32, text_x: f32, hs: f32, cw: f32, line: &str) -> usize {
    ((((mx - ex - text_x + hs) / cw).floor().max(0.0)) as usize).min(line.len())
}

// Convert a mouse Y position into a row index clamped to valid lines.
fn mouse_row(my: f32, ey: f32, text_y: f32, gs: f32, lh: f32, line_count: usize) -> usize {
    let row_float = ((my - ey - text_y + gs) / lh).floor().max(0.0) as usize;
    row_float.min(line_count.saturating_sub(1))
}

// Return the index of the last fully-visible line inside the editor viewport.
fn last_visible_row(gs: f32, text_y: f32, lh: f32, eh: f32, line_count: usize) -> usize {
    let viewport_bottom = gs + eh - text_y;
    let last = ((viewport_bottom / lh).floor() as isize - 1).max(0) as usize;
    last.min(line_count.saturating_sub(1))
}

// Return the index of the first fully-visible line inside the editor viewport.
fn first_visible_row(gs: f32, text_y: f32, lh: f32) -> usize {
    ((gs / lh).ceil() as isize).max(0) as usize
}

pub fn register(cv: &mut Canvas, ed: &Editor) {

    // ── Key handler ───────────────────────────────────────────────────────────
    let state_k         = ed.state.clone();
    let cfg_k           = ed.cfg.clone();
    let idle_k          = ed.idle_timer.clone();
    let cursor_vis_k    = ed.cursor_vis.clone();
    let blink_k         = ed.blink_timer.clone();
    let live_x_k        = ed.live_x.clone();
    let live_y_k        = ed.live_y.clone();
    let live_w_k        = ed.live_w.clone();
    let live_h_k        = ed.live_h.clone();
    let scroll_k        = ed.global_scroll.clone();
    let scroll_intent_k = ed.scroll_intent.clone();

    cv.on_key_press(move |cv, key| {
        let ex = { *live_x_k.get() }; let ey = { *live_y_k.get() };
        let ew = { *live_w_k.get() }; let eh = { *live_h_k.get() };
        if let Some((mx, my)) = cv.mouse_position() {
            if mx < ex || mx > ex + ew || my < ey || my > ey + eh { return; }
        } else { return; }

        *idle_k.get_mut()       = 0.0;
        *blink_k.get_mut()      = 0.0;
        *cursor_vis_k.get_mut() = true;

        let ctrl  = cv.is_key_held(&Key::Named(NamedKey::Control));
        let shift = cv.is_key_held(&Key::Named(NamedKey::Shift));

        let auto_pairs               = cfg_k.get().auto_pairs;
        let backspace_deletes_before = cfg_k.get().backspace_deletes_before;

        let mut st = state_k.lock().unwrap();
        if st.lines.is_empty() { return; }
        let mut changed = true;

        // ── Modifier combos ───────────────────────────────────────────────────
        if ctrl {
            match key {
                Key::Character(ch) => match ch.to_lowercase().as_str() {
                    "s" => { st.save(); }
                    "z" => {
                        if shift { println!("Ctrl+Shift+Z (redo)"); }
                        else     { println!("Ctrl+Z (undo)"); }
                    }
                    "c" => {
                        let txt = st.selected_text();
                        if !txt.is_empty() { println!("[copy] {txt:?}"); }
                    }
                    "x" => { println!("Ctrl+X (cut)"); }
                    "v" => { println!("Ctrl+V (paste)"); }
                    "a" => {
                        if !st.lines.is_empty() {
                            st.sel_anchor = Some((0, 0));
                            let last_row = st.lines.len() - 1;
                            st.sel_active = Some((last_row, st.lines[last_row].len()));
                            let txt = st.selected_text();
                            println!("[selected] {txt:?}");
                        }
                    }
                    "d" => { println!("Ctrl+D (duplicate line)"); }
                    "/" => {
                        let row = st.cursor_row;
                        let line = &st.lines[row];
                        let trimmed = line.trim_start();
                        if trimmed.starts_with("//") {
                            let slash_pos = line.find("//").unwrap();
                            st.lines[row].replace_range(slash_pos..slash_pos + 2, "");
                        } else {
                            st.lines[row].insert_str(0, "//");
                        }
                        st.dirty = true;
                        st.clear_selection();
                    }
                    _ => {}
                }
                Key::Named(NamedKey::Home)     => { st.cursor_row = 0; st.cursor_col = 0; st.clear_selection(); }
                Key::Named(NamedKey::End)      => {
                    let last = st.lines.len() - 1;
                    st.cursor_row = last; st.cursor_col = st.lines[last].len();
                    st.clear_selection();
                }
                Key::Named(NamedKey::ArrowUp)  => { println!("Ctrl+Up (move line up)"); }
                Key::Named(NamedKey::ArrowDown) => { println!("Ctrl+Down (move line down)"); }
                _ => {}
            }
            return;
        }

        // ── Bare / Shift keys ─────────────────────────────────────────────────
        //
        // Selection rules:
        //   Shift + Arrow/Home/End  →  extend selection from anchor
        //   Any other key           →  clear selection first
        //
        // For Shift+Arrow we set the anchor on first press (if not already set),
        // move the cursor, then update sel_active to the new cursor position.

        match key {
            // ── Arrow keys ────────────────────────────────────────────────────
            Key::Named(NamedKey::ArrowUp) => {
                if shift {
                    if st.sel_anchor.is_none() { st.anchor_at_cursor(); }
                    if st.cursor_row > 0 { st.cursor_row -= 1; st.clamp_col(); }
                    st.extend_selection_to_cursor();
                    print_selection(&st);
                } else {
                    st.clear_selection();
                    if st.cursor_row > 0 { st.cursor_row -= 1; st.clamp_col(); } else { changed = false; }
                }

                // Set scroll_intent so the tick loop drives scroll_vel every
                // frame for as long as the key is held.  Once the cursor is
                // above the first visible row, clear intent so normal arrow
                // navigation does not keep scrolling.
                {
                    let cfg       = *cfg_k.get();
                    let gs        = { *scroll_k.get() };
                    let lh        = cfg.line_height();
                    let first_vis = first_visible_row(gs, cfg.text_y, lh);
                    if st.cursor_row <= first_vis {
                        *scroll_intent_k.get_mut() = -cfg.scroll_max;
                    } else {
                        *scroll_intent_k.get_mut() = 0.0;
                    }
                }
            }
            Key::Named(NamedKey::ArrowDown) => {
                if shift {
                    if st.sel_anchor.is_none() { st.anchor_at_cursor(); }
                    if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.clamp_col(); }
                    st.extend_selection_to_cursor();
                    print_selection(&st);
                } else {
                    st.clear_selection();
                    if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.clamp_col(); } else { changed = false; }
                }

                {
                    let cfg      = *cfg_k.get();
                    let gs       = { *scroll_k.get() };
                    let lh       = cfg.line_height();
                    let last_vis = last_visible_row(gs, cfg.text_y, lh, eh, st.lines.len());
                    if st.cursor_row >= last_vis {
                        *scroll_intent_k.get_mut() = cfg.scroll_max;
                    } else {
                        *scroll_intent_k.get_mut() = 0.0;
                    }
                }
            }
            Key::Named(NamedKey::ArrowLeft) => {
                // Any non-vertical key clears intent so keyboard scrolling stops.
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel_anchor.is_none() { st.anchor_at_cursor(); }
                    if st.cursor_col > 0 { st.cursor_col -= 1; }
                    else if st.cursor_row > 0 { st.cursor_row -= 1; st.cursor_col = st.lines[st.cursor_row].len(); }
                    st.extend_selection_to_cursor();
                    print_selection(&st);
                } else {
                    st.clear_selection();
                    if st.cursor_col > 0 { st.cursor_col -= 1; }
                    else if st.cursor_row > 0 { st.cursor_row -= 1; st.cursor_col = st.lines[st.cursor_row].len(); }
                    else { changed = false; }
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel_anchor.is_none() { st.anchor_at_cursor(); }
                    let row = st.cursor_row;
                    if st.cursor_col < st.lines[row].len() { st.cursor_col += 1; }
                    else if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.cursor_col = 0; }
                    st.extend_selection_to_cursor();
                    print_selection(&st);
                } else {
                    st.clear_selection();
                    let row = st.cursor_row;
                    if st.cursor_col < st.lines[row].len() { st.cursor_col += 1; }
                    else if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.cursor_col = 0; }
                    else { changed = false; }
                }
            }

            // ── Home / End ────────────────────────────────────────────────────
            Key::Named(NamedKey::Home) => {
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel_anchor.is_none() { st.anchor_at_cursor(); }
                    st.cursor_col = 0;
                    st.extend_selection_to_cursor();
                    print_selection(&st);
                } else {
                    st.clear_selection();
                    st.cursor_col = 0;
                }
            }
            Key::Named(NamedKey::End) => {
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel_anchor.is_none() { st.anchor_at_cursor(); }
                    let r = st.cursor_row; st.cursor_col = st.lines[r].len();
                    st.extend_selection_to_cursor();
                    print_selection(&st);
                } else {
                    st.clear_selection();
                    let r = st.cursor_row; st.cursor_col = st.lines[r].len();
                }
            }

            // ── Editing keys (always clear selection + intent) ────────────────
            Key::Named(NamedKey::Space) => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let (row, col) = (st.cursor_row, st.cursor_col);
                st.lines[row].insert(col, ' '); st.cursor_col += 1; st.dirty = true;
            }
            Key::Named(NamedKey::Enter) => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let (row, col) = (st.cursor_row, st.cursor_col);
                let rest   = st.lines[row].split_off(col);
                let indent = st.lines[row].find(|c: char| !c.is_whitespace()).unwrap_or(0);
                let prev_char = st.lines[row].chars().last();
                let next_char = rest.chars().next();
                let between_pair = matches!(
                    (prev_char, next_char),
                    (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}'))
                );
                if between_pair {
                    let inner_indent = indent + 4;
                    st.lines.insert(row + 1, " ".repeat(inner_indent));
                    st.lines.insert(row + 2, " ".repeat(indent) + &rest);
                    st.cursor_row += 1; st.cursor_col = inner_indent;
                } else {
                    st.lines.insert(row + 1, " ".repeat(indent) + &rest);
                    st.cursor_row += 1; st.cursor_col = indent;
                }
                st.dirty = true;
            }
            Key::Named(NamedKey::Delete) => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let (row, col) = (st.cursor_row, st.cursor_col);
                if auto_pairs && col > 0 {
                    let prev = st.char_before(row, col);
                    let next = st.char_at(row, col);
                    if let (Some(p), Some(n)) = (prev, next) {
                        if is_auto_pair(p, n) {
                            st.lines[row].remove(col); st.lines[row].remove(col - 1);
                            st.cursor_col -= 1; st.dirty = true; return;
                        }
                    }
                }
                if backspace_deletes_before {
                    if col > 0 { st.lines[row].remove(col - 1); st.cursor_col -= 1; }
                    else if row > 0 {
                        let cur = st.lines.remove(row); st.cursor_row -= 1;
                        let nr = st.cursor_row; st.cursor_col = st.lines[nr].len();
                        st.lines[nr].push_str(&cur);
                    } else { changed = false; }
                } else {
                    if col < st.lines[row].len() { st.lines[row].remove(col); }
                    else if row + 1 < st.lines.len() {
                        let n = st.lines.remove(row + 1); st.lines[row].push_str(&n);
                    } else { changed = false; }
                }
                if changed { st.dirty = true; }
            }
            Key::Named(NamedKey::Tab) => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let (row, col) = (st.cursor_row, st.cursor_col);
                st.lines[row].insert_str(col, "    "); st.cursor_col += 4; st.dirty = true;
            }
            Key::Character(ch) if ch.len() == 1 => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let c          = ch.chars().next().unwrap();
                let (row, col) = (st.cursor_row, st.cursor_col);
                let next_char  = st.char_at(row, col);
                if auto_pairs && matches!(c, ')' | ']' | '}' | '"') && next_char == Some(c) {
                    st.cursor_col += 1;
                } else {
                    st.lines[row].insert(col, c); st.cursor_col += 1;
                    let new_col = st.cursor_col;
                    if auto_pairs {
                        if let Some(close) = auto_pair_close(c) { st.lines[row].insert(new_col, close); }
                    }
                    st.dirty = true;
                }
            }
            _ => { changed = false; }
        }
        let _ = changed;
    });

    // ── Mouse press ───────────────────────────────────────────────────────────
    let state_m      = ed.state.clone();
    let scroll_m     = ed.global_scroll.clone();
    let h_scroll_m   = ed.h_scroll.clone();
    let cfg_m        = ed.cfg.clone();
    let idle_m       = ed.idle_timer.clone();
    let cursor_vis_m = ed.cursor_vis.clone();
    let blink_m      = ed.blink_timer.clone();
    let dragging_m   = ed.dragging.clone();
    let live_x_m     = ed.live_x.clone();
    let live_y_m     = ed.live_y.clone();
    let live_w_m     = ed.live_w.clone();
    let live_h_m     = ed.live_h.clone();

    cv.on_mouse_press(move |_cv, button, (mx, my)| {
        if button != MouseButton::Left { return; }
        let ex = { *live_x_m.get() }; let ey = { *live_y_m.get() };
        let ew = { *live_w_m.get() }; let eh = { *live_h_m.get() };
        if mx < ex || mx > ex + ew || my < ey || my > ey + eh { return; }
        *idle_m.get_mut()       = 0.0;
        *blink_m.get_mut()      = 0.0;
        *cursor_vis_m.get_mut() = true;
        let cfg = *cfg_m.get();
        let gs  = { *scroll_m.get() };
        let hs  = { *h_scroll_m.get() };
        let mut st = state_m.lock().unwrap();
        if st.lines.is_empty() { return; }
        let row = mouse_row(my, ey, cfg.text_y, gs, cfg.line_height(), st.lines.len());
        let col = mouse_col(mx, ex, cfg.text_x, hs, cfg.char_width(), &st.lines[row]);
        st.cursor_row = row; st.cursor_col = col;
        st.sel_anchor = Some((row, col));
        st.sel_active = Some((row, col));
        *dragging_m.get_mut() = true;
    });

    // ── Mouse move (drag-select + edge auto-scroll) ───────────────────────────
    let state_mv         = ed.state.clone();
    let scroll_mv        = ed.global_scroll.clone();
    let h_scroll_mv      = ed.h_scroll.clone();
    let scroll_vel_mv    = ed.scroll_vel.clone();
    let scroll_intent_mv = ed.scroll_intent.clone();
    let cfg_mv           = ed.cfg.clone();
    let dragging_mv      = ed.dragging.clone();
    let live_x_mv        = ed.live_x.clone();
    let live_y_mv        = ed.live_y.clone();
    let live_w_mv        = ed.live_w.clone();
    let live_h_mv        = ed.live_h.clone();

    cv.on_mouse_move(move |_cv, (mx, my)| {
        if !{ *dragging_mv.get() } { return; }

        let ex = { *live_x_mv.get() }; let ey = { *live_y_mv.get() };
        let ew = { *live_w_mv.get() }; let eh = { *live_h_mv.get() };
        let cfg = *cfg_mv.get();
        let lh  = cfg.line_height();

        if my < ey {
            // Above editor — overshoot-scaled impulse + matching intent so
            // the tick loop sustains it if the mouse stops outside the window.
            let overshoot = ey - my;
            let lines_out = (overshoot / lh).min(8.0);
            let vel       = -(lines_out * lh * 0.6).min(cfg.scroll_max);
            *scroll_vel_mv.get_mut()    = vel;
            *scroll_intent_mv.get_mut() = vel;
        } else if my > ey + eh {
            let overshoot = my - (ey + eh);
            let lines_out = (overshoot / lh).min(8.0);
            let vel       = (lines_out * lh * 0.6).min(cfg.scroll_max);
            *scroll_vel_mv.get_mut()    = vel;
            *scroll_intent_mv.get_mut() = vel;
        } else {
            // Mouse is inside the editor.  Set intent based on whether the
            // cursor has landed on the first or last visible row so the tick
            // loop keeps scrolling even when the mouse stops moving.
            let gs = { *scroll_mv.get() };
            let line_count = {
                let st = state_mv.lock().unwrap();
                st.lines.len()
            };
            let row       = mouse_row(my, ey, cfg.text_y, gs, lh, line_count);
            let last_vis  = last_visible_row(gs, cfg.text_y, lh, eh, line_count);
            let first_vis = first_visible_row(gs, cfg.text_y, lh);

            if row >= last_vis {
                *scroll_intent_mv.get_mut() = cfg.scroll_max * 0.5;
            } else if row <= first_vis && first_vis > 0 {
                *scroll_intent_mv.get_mut() = -(cfg.scroll_max * 0.5);
            } else {
                // Cursor is safely mid-viewport — stop sustained scrolling.
                *scroll_intent_mv.get_mut() = 0.0;
            }
        }

        // Update selection using clamped coordinates so it extends to the edge
        // even when the mouse is outside the editor bounds.
        let mx_c = mx.clamp(ex, ex + ew);
        let my_c = my.clamp(ey, ey + eh);
        let gs   = { *scroll_mv.get() };
        let hs   = { *h_scroll_mv.get() };
        let mut st = state_mv.lock().unwrap();
        if st.lines.is_empty() { return; }
        let row = mouse_row(my_c, ey, cfg.text_y, gs, lh, st.lines.len());
        let col = mouse_col(mx_c, ex, cfg.text_x, hs, cfg.char_width(), &st.lines[row]);
        st.cursor_row = row;
        st.cursor_col = col;
        st.sel_active = Some((row, col));
        let txt = st.selected_text();
        if !txt.is_empty() { println!("[selecting] {txt:?}"); }
    });

    // ── Mouse release ─────────────────────────────────────────────────────────
    let state_mr         = ed.state.clone();
    let dragging_mr      = ed.dragging.clone();
    let scroll_vel_mr    = ed.scroll_vel.clone();
    let scroll_intent_mr = ed.scroll_intent.clone();
    let live_x_mr        = ed.live_x.clone();
    let live_y_mr        = ed.live_y.clone();
    let live_w_mr        = ed.live_w.clone();
    let live_h_mr        = ed.live_h.clone();

    cv.on_mouse_release(move |_cv, button, (mx, my)| {
        if button != MouseButton::Left { return; }
        if !{ *dragging_mr.get() } { return; }
        let ex = { *live_x_mr.get() }; let ey = { *live_y_mr.get() };
        let ew = { *live_w_mr.get() }; let eh = { *live_h_mr.get() };
        let _ = (mx, my, ex, ey, ew, eh);
        *dragging_mr.get_mut()      = false;
        // Stop all sustained and momentum scroll on release.
        *scroll_vel_mr.get_mut()    = 0.0;
        *scroll_intent_mr.get_mut() = 0.0;
        let st = state_mr.lock().unwrap();
        let txt = st.selected_text();
        if txt.is_empty() { println!("[selection] (none)"); }
        else              { println!("[selected] {txt:?}"); }
    });

    // ── Mouse scroll ──────────────────────────────────────────────────────────
    let scroll_vel_s     = ed.scroll_vel.clone();
    let h_scroll_vel_s   = ed.h_scroll_vel.clone();
    let h_scroll_s       = ed.h_scroll.clone();
    let max_line_width_s = ed.max_line_width.clone();
    let cfg_s            = ed.cfg.clone();
    let live_x_s = ed.live_x.clone(); let live_y_s = ed.live_y.clone();
    let live_w_s = ed.live_w.clone(); let live_h_s = ed.live_h.clone();

    cv.on_mouse_scroll(move |cv, (dx, dy)| {
        if let Some((mx, my)) = cv.mouse_position() {
            let ex = { *live_x_s.get() }; let ey = { *live_y_s.get() };
            let ew = { *live_w_s.get() }; let eh = { *live_h_s.get() };
            if mx < ex || mx > ex + ew || my < ey || my > ey + eh { return; }
        } else { return; }
        let cfg = *cfg_s.get();
        if dy != 0.0 {
            let dir  = if dy > 0.0 { 1.0f32 } else { -1.0 };
            let push = (dy.abs() * cfg.scroll_accel).min(cfg.scroll_max);
            let cur  = { *scroll_vel_s.get() };
            *scroll_vel_s.get_mut() = if cur == 0.0 || cur.signum() == dir {
                (cur + dir * push).clamp(-cfg.scroll_max, cfg.scroll_max)
            } else { dir * push };
        }
        if dx != 0.0 {
            let dir     = if dx > 0.0 { 1.0f32 } else { -1.0 };
            let push    = (dx.abs() * cfg.scroll_accel).min(cfg.scroll_max);
            let cur_vel = { *h_scroll_vel_s.get() };
            let cur_pos = { *h_scroll_s.get() };
            let code_w  = { *live_w_s.get() } - cfg.text_x - RIGHT_PAD;
            let h_max   = { *max_line_width_s.get() } - code_w;
            let new_vel = if cur_vel == 0.0 || cur_vel.signum() == dir {
                (cur_vel + dir * push).clamp(-cfg.scroll_max, cfg.scroll_max)
            } else { dir * push };
            let clamped = if cur_pos <= 0.0 && new_vel < 0.0 { 0.0 }
                     else if h_max > 0.0 && cur_pos >= h_max && new_vel > 0.0 { 0.0 }
                     else { new_vel };
            *h_scroll_vel_s.get_mut() = clamped;
        }
    });
}

// ── Helpers ───────────────────────────────────────────────────────────────────

fn print_selection(st: &crate::editor::EditorState) {
    let txt = st.selected_text();
    if !txt.is_empty() { println!("[selected] {txt:?}"); }
}