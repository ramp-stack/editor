use flowmango::Canvas;
use quartz::{Key, MouseButton, NamedKey};

use crate::components::auto_pairs;
use crate::components::minimap::MINIMAP_W;
use crate::components::scroll::ScrollAxis;
use crate::components::selection;
use crate::constants::RIGHT_PAD;
use crate::editor::Editor;
use crate::objects::editor_obj::SCROLLBAR_W;

fn mouse_col(mx: f32, ex: f32, text_x: f32, hs: f32, cw: f32, line: &str) -> usize {
    let char_count = selection::char_len(line);
    (((mx - ex - text_x + hs) / cw).floor().max(0.0) as usize).min(char_count)
}

fn mouse_row(my: f32, ey: f32, text_y: f32, gs: f32, lh: f32, line_count: usize) -> usize {
    let r = ((my - ey - text_y + gs) / lh).floor().max(0.0) as usize;
    r.min(line_count.saturating_sub(1))
}

fn last_visible_row(gs: f32, text_y: f32, lh: f32, eh: f32, line_count: usize) -> usize {
    let bottom = gs + eh - text_y;
    let last   = ((bottom / lh).floor() as isize - 1).max(0) as usize;
    last.min(line_count.saturating_sub(1))
}

fn first_visible_row(gs: f32, _text_y: f32, lh: f32) -> usize {
    ((gs / lh).ceil() as isize).max(0) as usize
}

pub fn register(cv: &mut Canvas, ed: &Editor) {
    register_keys(cv, ed);
    register_mouse_press(cv, ed);
    register_mouse_move(cv, ed);
    register_mouse_release(cv, ed);
    register_mouse_scroll(cv, ed);
    register_scrollbar(cv, ed);
}

fn register_scrollbar(cv: &mut Canvas, ed: &Editor) {
    let v_scroll_sb   = ed.v_scroll.clone();
    let cfg_sb        = ed.cfg.clone();
    let live_x_sb     = ed.live_x.clone();
    let live_y_sb     = ed.live_y.clone();
    let live_w_sb     = ed.live_w.clone();
    let live_h_sb     = ed.live_h.clone();
    let state_sb      = ed.state.clone();
    let sb_dragging   = quartz::Shared::new(false);
    let sb_drag_off   = quartz::Shared::new(0.0f32);
    let sb_dragging_m = sb_dragging.clone();
    let sb_drag_off_m = sb_drag_off.clone();
    let sb_dragging_r = sb_dragging.clone();

    let v_scroll_sb_m = ed.v_scroll.clone();
    let cfg_sb_m      = ed.cfg.clone();
    let live_x_sb_m   = ed.live_x.clone();
    let live_y_sb_m   = ed.live_y.clone();
    let live_w_sb_m   = ed.live_w.clone();
    let live_h_sb_m   = ed.live_h.clone();
    let state_sb_m    = ed.state.clone();

    cv.on_mouse_press(move |_cv, button, (mx, my)| {
        if button != MouseButton::Left { return; }
        let ex   = { *live_x_sb.get() };
        let ew   = { *live_w_sb.get() };
        let ey   = { *live_y_sb.get() };
        let eh   = { *live_h_sb.get() };
        let sb_x = ex + ew - SCROLLBAR_W;

        if mx < sb_x || mx > sb_x + SCROLLBAR_W { return; }
        if my < ey   || my > ey + eh             { return; }

        let cfg        = *cfg_sb.get();
        let total      = { state_sb.lock().unwrap().lines.len().max(1) };
        let viewport_h = (eh - cfg.text_y).max(0.0);
        let lh         = cfg.line_height();
        let v_max      = (total as f32 * lh - viewport_h).max(0.0);
        let gs         = { v_scroll_sb.get().offset };
        let track_h    = eh;
        let thumb_h    = if v_max > 0.0 {
            (viewport_h / (viewport_h + v_max) * track_h).max(20.0).min(track_h)
        } else { track_h };
        let thumb_top  = if v_max > 0.0 {
            ey + (gs / v_max) * (track_h - thumb_h)
        } else { ey };

        if my >= thumb_top && my <= thumb_top + thumb_h {
            *sb_dragging.get_mut() = true;
            *sb_drag_off.get_mut() = my - thumb_top;
        } else {
            let center_off = my - ey - thumb_h * 0.5;
            let target_off = (center_off / (track_h - thumb_h)).clamp(0.0, 1.0) * v_max;
            v_scroll_sb.get_mut().jump_to(target_off, v_max);
            *sb_dragging.get_mut() = true;
            *sb_drag_off.get_mut() = thumb_h * 0.5;
        }
    });

    cv.on_mouse_move(move |_cv, (_mx, my)| {
        if !{ *sb_dragging_m.get() } { return; }
        let ey      = { *live_y_sb_m.get() };
        let eh      = { *live_h_sb_m.get() };
        let cfg     = *cfg_sb_m.get();
        let total   = { state_sb_m.lock().unwrap().lines.len().max(1) };
        let vp_h    = (eh - cfg.text_y).max(0.0);
        let lh      = cfg.line_height();
        let v_max   = (total as f32 * lh - vp_h).max(0.0);
        let track_h = eh;
        let thumb_h = if v_max > 0.0 {
            (vp_h / (vp_h + v_max) * track_h).max(20.0).min(track_h)
        } else { track_h };
        let drag_off   = { *sb_drag_off_m.get() };
        let thumb_top  = (my - ey - drag_off).clamp(0.0, track_h - thumb_h);
        let target_off = if track_h - thumb_h > 0.0 {
            (thumb_top / (track_h - thumb_h)) * v_max
        } else { 0.0 };
        v_scroll_sb_m.get_mut().jump_to(target_off, v_max);
    });

    cv.on_mouse_release(move |_cv, button, _pos| {
        if button != MouseButton::Left { return; }
        *sb_dragging_r.get_mut() = false;
    });
}

fn register_keys(cv: &mut Canvas, ed: &Editor) {
    let state_k         = ed.state.clone();
    let cfg_k           = ed.cfg.clone();
    let idle_k          = ed.idle_timer.clone();
    let cursor_vis_k    = ed.cursor_vis.clone();
    let blink_k         = ed.blink_timer.clone();
    let live_x_k        = ed.live_x.clone();
    let live_y_k        = ed.live_y.clone();
    let live_w_k        = ed.live_w.clone();
    let live_h_k        = ed.live_h.clone();
    let v_scroll_k      = ed.v_scroll.clone();
    let scroll_intent_k = ed.scroll_intent.clone();
    let focus_k         = ed.focus.clone();

    cv.on_key_press(move |cv, key| {
        if !{ *focus_k.get() } { return; }

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

        let cfg              = *cfg_k.get();
        let auto_pairs_on    = cfg.auto_pairs;
        let backspace_before = cfg.backspace_deletes_before;

        let mut st = state_k.lock().unwrap();
        if st.lines.is_empty() { return; }

        if ctrl {
            handle_ctrl_key(key, shift, &mut st);
            return;
        }

        match key {
            Key::Named(NamedKey::ArrowUp) => {
                if shift {
                    if st.sel.anchor.is_none() { st.anchor_at_cursor(); }
                    if st.cursor_row > 0 { st.cursor_row -= 1; st.clamp_col(); }
                    st.extend_selection_to_cursor();
                } else {
                    st.clear_selection();
                    if st.cursor_row > 0 { st.cursor_row -= 1; st.clamp_col(); }
                }
                let gs        = v_scroll_k.get().offset;
                let lh        = cfg.line_height();
                let first_vis = first_visible_row(gs, cfg.text_y, lh);
                *scroll_intent_k.get_mut() = if st.cursor_row <= first_vis { -cfg.scroll_max } else { 0.0 };
            }
            Key::Named(NamedKey::ArrowDown) => {
                if shift {
                    if st.sel.anchor.is_none() { st.anchor_at_cursor(); }
                    if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.clamp_col(); }
                    st.extend_selection_to_cursor();
                } else {
                    st.clear_selection();
                    if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.clamp_col(); }
                }
                let gs       = v_scroll_k.get().offset;
                let lh       = cfg.line_height();
                let last_vis = last_visible_row(gs, cfg.text_y, lh, eh, st.lines.len());
                *scroll_intent_k.get_mut() = if st.cursor_row >= last_vis { cfg.scroll_max } else { 0.0 };
            }
            Key::Named(NamedKey::ArrowLeft) => {
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel.anchor.is_none() { st.anchor_at_cursor(); }
                    if st.cursor_col > 0 { st.cursor_col -= 1; }
                    else if st.cursor_row > 0 {
                        st.cursor_row -= 1;
                        st.cursor_col = selection::char_len(&st.lines[st.cursor_row]);
                    }
                    st.extend_selection_to_cursor();
                } else {
                    st.clear_selection();
                    if st.cursor_col > 0 { st.cursor_col -= 1; }
                    else if st.cursor_row > 0 {
                        st.cursor_row -= 1;
                        st.cursor_col = selection::char_len(&st.lines[st.cursor_row]);
                    }
                }
            }
            Key::Named(NamedKey::ArrowRight) => {
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel.anchor.is_none() { st.anchor_at_cursor(); }
                    let row = st.cursor_row;
                    if st.cursor_col < selection::char_len(&st.lines[row]) { st.cursor_col += 1; }
                    else if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.cursor_col = 0; }
                    st.extend_selection_to_cursor();
                } else {
                    st.clear_selection();
                    let row = st.cursor_row;
                    if st.cursor_col < selection::char_len(&st.lines[row]) { st.cursor_col += 1; }
                    else if st.cursor_row + 1 < st.lines.len() { st.cursor_row += 1; st.cursor_col = 0; }
                }
            }
            Key::Named(NamedKey::Home) => {
                *scroll_intent_k.get_mut() = 0.0;
                if shift {
                    if st.sel.anchor.is_none() { st.anchor_at_cursor(); }
                    st.cursor_col = 0; st.extend_selection_to_cursor();
                } else { st.clear_selection(); st.cursor_col = 0; }
            }
            Key::Named(NamedKey::End) => {
                *scroll_intent_k.get_mut() = 0.0;
                let r = st.cursor_row;
                if shift {
                    if st.sel.anchor.is_none() { st.anchor_at_cursor(); }
                    st.cursor_col = selection::char_len(&st.lines[r]);
                    st.extend_selection_to_cursor();
                } else {
                    st.clear_selection();
                    st.cursor_col = selection::char_len(&st.lines[r]);
                }
            }
            Key::Named(NamedKey::Space) => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let (row, col) = (st.cursor_row, st.cursor_col);
                let byte = selection::char_col_to_byte_col(&st.lines[row], col);
                st.lines[row].insert(byte, ' ');
                st.cursor_col += 1;
                st.bump();
            }
            Key::Named(NamedKey::Enter) => {
                *scroll_intent_k.get_mut() = 0.0;
                st.clear_selection();
                let (row, col)  = (st.cursor_row, st.cursor_col);
                let byte        = selection::char_col_to_byte_col(&st.lines[row], col);
                let rest        = st.lines[row].split_off(byte);
                let indent      = st.lines[row].find(|c: char| !c.is_whitespace()).unwrap_or(0);
                let prev_char   = st.lines[row].chars().last();
                let next_char   = rest.chars().next();
                let between_pair = matches!(
                    (prev_char, next_char),
                    (Some('('), Some(')')) | (Some('['), Some(']')) | (Some('{'), Some('}'))
                );
                if between_pair {
                    let inner = indent + 4;
                    st.lines.insert(row + 1, " ".repeat(inner));
                    st.lines.insert(row + 2, " ".repeat(indent) + &rest);
                    st.cursor_row += 1; st.cursor_col = inner;
                } else {
                    st.lines.insert(row + 1, " ".repeat(indent) + &rest);
                    st.cursor_row += 1; st.cursor_col = indent;
                }
                st.bump();
            }
            Key::Named(NamedKey::Delete) | Key::Named(NamedKey::Backspace) => {
                *scroll_intent_k.get_mut() = 0.0;

                if st.sel.is_active() {
                    delete_selection(&mut st);
                    return;
                }

                st.clear_selection();
                let (row, col) = (st.cursor_row, st.cursor_col);

                let acts_as_backspace = matches!(key, Key::Named(NamedKey::Backspace))
                    || (matches!(key, Key::Named(NamedKey::Delete)) && backspace_before);

                if acts_as_backspace {
                    if auto_pairs_on && col > 0 {
                        let prev = st.char_before(row, col);
                        let next = st.char_at(row, col);
                        if auto_pairs::should_delete_pair(prev, next) {
                            let byte_next = selection::char_col_to_byte_col(&st.lines[row], col);
                            let byte_prev = selection::char_col_to_byte_col(&st.lines[row], col - 1);
                            st.lines[row].remove(byte_next);
                            st.lines[row].remove(byte_prev);
                            st.cursor_col -= 1;
                            st.bump();
                            return;
                        }
                    }
                    if col > 0 {
                        let byte = selection::char_col_to_byte_col(&st.lines[row], col - 1);
                        st.lines[row].remove(byte);
                        st.cursor_col -= 1;
                        st.bump();
                    } else if row > 0 {
                        let cur = st.lines.remove(row);
                        st.cursor_row -= 1;
                        let nr = st.cursor_row;
                        st.cursor_col = selection::char_len(&st.lines[nr]);
                        st.lines[nr].push_str(&cur);
                        st.bump();
                    }
                } else {
                    if col < selection::char_len(&st.lines[row]) {
                        let byte = selection::char_col_to_byte_col(&st.lines[row], col);
                        st.lines[row].remove(byte);
                        st.bump();
                    } else if row + 1 < st.lines.len() {
                        let n = st.lines.remove(row + 1);
                        st.lines[row].push_str(&n);
                        st.bump();
                    }
                }
            }
            Key::Named(NamedKey::Tab) => {
                *scroll_intent_k.get_mut() = 0.0;
                if st.sel.is_active() { delete_selection(&mut st); }
                else { st.clear_selection(); }
                let (row, col) = (st.cursor_row, st.cursor_col);
                let byte = selection::char_col_to_byte_col(&st.lines[row], col);
                st.lines[row].insert_str(byte, "    ");
                st.cursor_col += 4;
                st.bump();
            }
            Key::Character(ch) if ch.len() == 1 => {
                *scroll_intent_k.get_mut() = 0.0;
                if st.sel.is_active() { delete_selection(&mut st); }
                else { st.clear_selection(); }
                let c          = ch.chars().next().unwrap();
                let (row, col) = (st.cursor_row, st.cursor_col);
                let next_char  = st.char_at(row, col);
                if auto_pairs_on && matches!(c, ')' | ']' | '}' | '"') && next_char == Some(c) {
                    st.cursor_col += 1;
                } else {
                    let byte = selection::char_col_to_byte_col(&st.lines[row], col);
                    st.lines[row].insert(byte, c);
                    st.cursor_col += 1;
                    if auto_pairs_on {
                        if let Some(close) = auto_pairs::pair_close(c) {
                            let new_byte = selection::char_col_to_byte_col(&st.lines[row], st.cursor_col);
                            st.lines[row].insert(new_byte, close);
                        }
                    }
                    st.bump();
                }
            }
            _ => {}
        }
    });
}

fn handle_ctrl_key(key: &Key, shift: bool, st: &mut crate::editor::state::EditorState) {
    match key {
        Key::Character(ch) => match ch.to_lowercase().as_str() {
            "s" => { st.save(); }
            "z" => { if shift { println!("Ctrl+Shift+Z (redo)"); } else { println!("Ctrl+Z (undo)"); } }
            "c" => { let t = st.selected_text(); if !t.is_empty() { println!("[copy] {t:?}"); } }
            "x" => println!("Ctrl+X (cut)"),
            "v" => println!("Ctrl+V (paste)"),
            "a" => { st.select_all(); }
            "/" => {
                let row = st.cursor_row; let line = &st.lines[row];
                if line.trim_start().starts_with("//") {
                    let pos = line.find("//").unwrap();
                    st.lines[row].replace_range(pos..pos + 2, "");
                } else { st.lines[row].insert_str(0, "//"); }
                st.bump();
                st.clear_selection();
            }
            _ => {}
        }
        Key::Named(NamedKey::Home) => { st.cursor_row = 0; st.cursor_col = 0; st.clear_selection(); }
        Key::Named(NamedKey::End)  => {
            let last = st.lines.len() - 1;
            st.cursor_row = last;
            st.cursor_col = selection::char_len(&st.lines[last]);
            st.clear_selection();
        }
        _ => {}
    }
}

fn register_mouse_press(cv: &mut Canvas, ed: &Editor) {
    let state_m      = ed.state.clone();
    let v_scroll_m   = ed.v_scroll.clone();
    let h_scroll_m   = ed.h_scroll.clone();
    let cfg_m        = ed.cfg.clone();
    let idle_m       = ed.idle_timer.clone();
    let cursor_vis_m = ed.cursor_vis.clone();
    let blink_m      = ed.blink_timer.clone();
    let dragging_m   = ed.dragging.clone();
    let focus_m      = ed.focus.clone();
    let live_x_m = ed.live_x.clone(); let live_y_m = ed.live_y.clone();
    let live_w_m = ed.live_w.clone(); let live_h_m = ed.live_h.clone();

    cv.on_mouse_press(move |cv, button, (mx, my)| {
        if button != MouseButton::Left { return; }
        let ex = { *live_x_m.get() }; let ey = { *live_y_m.get() };
        let ew = { *live_w_m.get() }; let eh = { *live_h_m.get() };

        if mx < ex || mx > ex + ew || my < ey || my > ey + eh {
            *focus_m.get_mut() = false;
            return;
        }

        let mmx = ex + ew - MINIMAP_W;
        if mx >= mmx { return; }

        *focus_m.get_mut()      = true;
        *idle_m.get_mut()       = 0.0;
        *blink_m.get_mut()      = 0.0;
        *cursor_vis_m.get_mut() = true;

        let shift = cv.is_key_held(&Key::Named(NamedKey::Shift));
        let cfg   = *cfg_m.get();
        let gs    = v_scroll_m.get().offset;
        let hs    = h_scroll_m.get().offset;
        let mut st = state_m.lock().unwrap();
        if st.lines.is_empty() { return; }

        let row = mouse_row(my, ey, cfg.text_y, gs, cfg.line_height(), st.lines.len());
        let col = mouse_col(mx, ex, cfg.text_x, hs, cfg.char_width(), &st.lines[row]);
        st.cursor_row = row; st.cursor_col = col;

        if shift && st.sel.anchor.is_some() {
            st.sel.extend((row, col));
        } else {
            st.sel.begin((row, col));
        }

        *dragging_m.get_mut() = true;
    });
}

fn register_mouse_move(cv: &mut Canvas, ed: &Editor) {
    let state_mv         = ed.state.clone();
    let v_scroll_mv      = ed.v_scroll.clone();
    let h_scroll_mv      = ed.h_scroll.clone();
    let scroll_intent_mv = ed.scroll_intent.clone();
    let cfg_mv           = ed.cfg.clone();
    let dragging_mv      = ed.dragging.clone();
    let live_x_mv = ed.live_x.clone(); let live_y_mv = ed.live_y.clone();
    let live_w_mv = ed.live_w.clone(); let live_h_mv = ed.live_h.clone();

    cv.on_mouse_move(move |_cv, (mx, my)| {
        if !{ *dragging_mv.get() } { return; }

        let ex = { *live_x_mv.get() }; let ey = { *live_y_mv.get() };
        let ew = { *live_w_mv.get() }; let eh = { *live_h_mv.get() };
        let cfg = *cfg_mv.get();
        let lh  = cfg.line_height();

        if my < ey {
            let overshoot = ey - my;
            let vel       = -((overshoot / lh).min(8.0) * lh * 0.6).min(cfg.scroll_max);
            v_scroll_mv.get_mut().velocity = vel;
            *scroll_intent_mv.get_mut()    = vel;
        } else if my > ey + eh {
            let overshoot = my - (ey + eh);
            let vel       = ((overshoot / lh).min(8.0) * lh * 0.6).min(cfg.scroll_max);
            v_scroll_mv.get_mut().velocity = vel;
            *scroll_intent_mv.get_mut()    = vel;
        } else {
            let gs         = v_scroll_mv.get().offset;
            let line_count = { state_mv.lock().unwrap().lines.len() };
            let row        = mouse_row(my, ey, cfg.text_y, gs, lh, line_count);
            let last_vis   = last_visible_row(gs, cfg.text_y, lh, eh, line_count);
            let first_vis  = first_visible_row(gs, cfg.text_y, lh);
            *scroll_intent_mv.get_mut() = if row >= last_vis {
                cfg.scroll_max * 0.5
            } else if row <= first_vis && first_vis > 0 {
                -(cfg.scroll_max * 0.5)
            } else { 0.0 };
        }

        let mx_c = mx.clamp(ex, ex + ew);
        let my_c = my.clamp(ey, ey + eh);
        let gs   = v_scroll_mv.get().offset;
        let hs   = h_scroll_mv.get().offset;
        let mut st = state_mv.lock().unwrap();
        if st.lines.is_empty() { return; }
        let row = mouse_row(my_c, ey, cfg.text_y, gs, lh, st.lines.len());
        let col = mouse_col(mx_c, ex, cfg.text_x, hs, cfg.char_width(), &st.lines[row]);
        st.cursor_row = row; st.cursor_col = col;
        st.sel.extend((row, col));
    });
}

fn register_mouse_release(cv: &mut Canvas, ed: &Editor) {
    let dragging_mr      = ed.dragging.clone();
    let v_scroll_mr      = ed.v_scroll.clone();
    let scroll_intent_mr = ed.scroll_intent.clone();

    cv.on_mouse_release(move |_cv, button, _pos| {
        if button != MouseButton::Left { return; }
        if !{ *dragging_mr.get() }     { return; }
        *dragging_mr.get_mut()         = false;
        v_scroll_mr.get_mut().velocity = 0.0;
        *scroll_intent_mr.get_mut()    = 0.0;
    });
}

fn register_mouse_scroll(cv: &mut Canvas, ed: &Editor) {
    let v_scroll_s       = ed.v_scroll.clone();
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
            let dir  = dy.signum();
            let push = (dy.abs() * cfg.scroll_accel).min(cfg.scroll_max);
            v_scroll_s.get_mut().push(dir * push, cfg.scroll_max);
        }
        if dx != 0.0 {
            let dir    = dx.signum();
            let push   = (dx.abs() * cfg.scroll_accel).min(cfg.scroll_max);
            let code_w = { *live_w_s.get() } - cfg.text_x - RIGHT_PAD;
            let h_max  = ({ *max_line_width_s.get() } - code_w).max(0.0);
            let ax     = h_scroll_s.get();
            if (ax.offset <= 0.0 && dir < 0.0) || (h_max > 0.0 && ax.offset >= h_max && dir > 0.0) {
                return;
            }
            drop(ax);
            h_scroll_s.get_mut().push(dir * push, cfg.scroll_max);
        }
    });
}

fn delete_selection(st: &mut crate::editor::state::EditorState) {
    let Some(((r1, c1), (r2, c2))) = st.sel.range() else { return; };

    if r1 == r2 {
        let b1 = selection::char_col_to_byte_col(&st.lines[r1], c1);
        let b2 = selection::char_col_to_byte_col(&st.lines[r1], c2);
        st.lines[r1].replace_range(b1..b2, "");
    } else {
        let b1   = selection::char_col_to_byte_col(&st.lines[r1], c1);
        let b2   = selection::char_col_to_byte_col(&st.lines[r2], c2);
        let tail = st.lines[r2][b2..].to_string();
        st.lines[r1].truncate(b1);
        st.lines[r1].push_str(&tail);
        st.lines.drain((r1 + 1)..=r2);
    }

    st.cursor_row = r1;
    st.cursor_col = c1;
    st.sel.clear();
    st.bump();
}