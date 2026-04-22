use quartz::tint_overlay;
use ramp::prism;
use flowmango::Canvas;

use crate::constants::*;
use crate::editor::Editor;
use crate::editor::ObjNames;
use crate::editor::{SEL_OVERLAY_COUNT, sel_overlay_name};
use crate::editor::viewer::{FileMode, update_image_obj, show_text_mode, show_image_mode};
use crate::editor::highlight::{build_text_slice, build_gutter_slice};

pub fn register(cv: &mut Canvas, ed: &Editor) {
    let cfg_u            = ed.cfg.clone();
    let theme_u          = ed.theme.clone();
    let code_font_u      = ed.code_font.clone();
    let gutter_font_u    = ed.gutter_font.clone();
    let scroll_u         = ed.global_scroll.clone();
    let h_scroll_u       = ed.h_scroll.clone();
    let scroll_vel_u     = ed.scroll_vel.clone();
    let h_scroll_vel_u   = ed.h_scroll_vel.clone();
    let max_line_width_u = ed.max_line_width.clone();
    let state_u          = ed.state.clone();
    let blink_timer      = ed.blink_timer.clone();
    let idle_timer       = ed.idle_timer.clone();
    let cursor_vis       = ed.cursor_vis.clone();
    let autosave_timer   = ed.autosave_timer.clone();
    let live_x_u = ed.live_x.clone(); let live_y_u = ed.live_y.clone();
    let live_w_u = ed.live_w.clone(); let live_h_u = ed.live_h.clone();
    let names_u          = ObjNames::from_prefix(&ed.id_prefix);
    let id_prefix_u      = ed.id_prefix.clone();
    let img_loaded_key_u = ed.img_loaded_key.clone();
    let scroll_intent    = ed.scroll_intent.clone();
    let dragging_u       = ed.dragging.clone();

    cv.on_update(move |cv| {
        let cfg   = *cfg_u.get();
        let theme = theme_u.get().clone();
        let lh    = cfg.line_height();
        let cw    = cfg.char_width();
        let ex = { *live_x_u.get() }; let ey = { *live_y_u.get() };
        let ew = { *live_w_u.get() }; let eh = { *live_h_u.get() };
        let code_x = ex + cfg.text_x;
        let code_w = ew - cfg.text_x - RIGHT_PAD;
        let code_y = ey + cfg.text_y;

        let (cur_mode, cur_path, cur_lang) = {
            let st = state_u.lock().unwrap();
            (st.mode.clone(), st.path.clone(), st.lang.clone())
        };
        let is_image = cur_mode == FileMode::Image;

        if let Some(o) = cv.get_game_object_mut(&names_u.bg) {
            o.position = (ex, ey); o.size = (ew, eh);
            o.set_image(tint_overlay(4000.0, 4000.0, theme.editor_bg));
        }

        if is_image {
            update_image_obj(cv, &id_prefix_u, &cur_path, ex, ey, ew, eh, &img_loaded_key_u);
            show_image_mode(cv, &id_prefix_u, &[
                names_u.gutter_bg.as_str(), names_u.gutter.as_str(),
                names_u.code_text.as_str(), names_u.cursor.as_str(),
            ]);
            // Hide all selection overlays in image mode
            for i in 0..SEL_OVERLAY_COUNT {
                let name = sel_overlay_name(&id_prefix_u, i);
                if let Some(o) = cv.get_game_object_mut(&name) { o.visible = false; }
            }
            return;
        }

        show_text_mode(cv, &id_prefix_u, &[
            names_u.gutter_bg.as_str(), names_u.gutter.as_str(),
            names_u.code_text.as_str(), names_u.cursor.as_str(),
        ]);

        {
            let timer = { *autosave_timer.get() };
            *autosave_timer.get_mut() = timer + 1.0 / 60.0;
            if timer + 1.0 / 60.0 >= AUTOSAVE_INTERVAL_SECS {
                *autosave_timer.get_mut() = 0.0;
                let mut st = state_u.lock().unwrap();
                if st.dirty { st.save(); }
            }
        }

        let (total, cur_row, cur_col) = {
            let st = state_u.lock().unwrap();
            (st.lines.len(), st.cursor_row, st.cursor_col)
        };
        if total == 0 { return; }

        {
            let cur_max = { *max_line_width_u.get() };
            if cur_max <= 0.0 {
                let st2 = state_u.lock().unwrap();
                if let Some(longest) = st2.lines.iter().max_by_key(|l| l.chars().count()) {
                    let t = build_text_slice(std::slice::from_ref(longest), &code_font_u, &cfg, &theme, &cur_lang);
                    *max_line_width_u.get_mut() = t.size().0;
                }
            }
        }

        {
            let intent = { *scroll_intent.get() };
            if intent != 0.0 {
                *scroll_vel_u.get_mut() = intent;
            }
        }

        {
            let vel = { *scroll_vel_u.get() };
            if vel.abs() > 0.1 {
                let v_max = ((total as f32 - 1.0) * lh).max(0.0);
                let cur   = { *scroll_u.get() };
                let next  = (cur + vel).clamp(0.0, v_max);
                let new_vel = if next <= 0.0 || next >= v_max { 0.0 } else { vel * cfg.scroll_friction };
                *scroll_u.get_mut()     = next;
                *scroll_vel_u.get_mut() = new_vel;
            } else { *scroll_vel_u.get_mut() = 0.0; }
        }

        {
            let h_max = ({ *max_line_width_u.get() } - code_w).max(0.0);
            let h_vel = { *h_scroll_vel_u.get() };
            if h_vel.abs() > 0.1 {
                let cur     = { *h_scroll_u.get() };
                let next    = (cur + h_vel).clamp(0.0, h_max);
                let new_vel = if next <= 0.0 || next >= h_max { 0.0 } else { h_vel * cfg.scroll_friction };
                *h_scroll_u.get_mut()     = next;
                *h_scroll_vel_u.get_mut() = new_vel;
            } else {
                *h_scroll_vel_u.get_mut() = 0.0;
                let cur = { *h_scroll_u.get() };
                if cur > ({ *max_line_width_u.get() } - code_w).max(0.0) {
                    *h_scroll_u.get_mut() = ({ *max_line_width_u.get() } - code_w).max(0.0);
                }
            }
        }

        let gs              = { *scroll_u.get() };
        let first_visible   = (gs / lh).floor() as usize;
        let sub_line_offset = gs - first_visible as f32 * lh;
        let (slice_start, text_top) = if first_visible > 0 {
            (first_visible - 1, code_y - sub_line_offset - lh)
        } else {
            (0, code_y - sub_line_offset)
        };

        let visible_lines = if eh > cfg.text_y {
            ((eh - cfg.text_y) / lh).ceil() as usize + 2
        } else { 2 };
        let last_visible = (slice_start + visible_lines).min(total);
        let hs = { *h_scroll_u.get() };
        
        {
            if *dragging_u.get() {
                if let Some((mx, my)) = cv.mouse_position() {
                    let mx_c = mx.clamp(ex, ex + ew);
                    let my_c = my.clamp(ey, ey + eh);
                    let mut st = state_u.lock().unwrap();
                    if !st.lines.is_empty() {
                        let row = {
                            let row_float = ((my_c - ey - cfg.text_y + gs) / lh)
                                .floor()
                                .max(0.0) as usize;
                            row_float.min(st.lines.len().saturating_sub(1))
                        };
                        let col = {
                            (((mx_c - ex - cfg.text_x + hs) / cw)
                                .floor()
                                .max(0.0) as usize)
                                .min(st.lines[row].len())
                        };
                        st.cursor_row = row;
                        st.cursor_col = col;
                        st.sel_active = Some((row, col));
                    }
                }
            }
        }

        {
            let st = state_u.lock().unwrap();
            if last_visible > slice_start {
                let text   = build_text_slice(&st.lines[slice_start..last_visible], &code_font_u, &cfg, &theme, &cur_lang);
                let gutter = build_gutter_slice(slice_start, last_visible, cur_row, &gutter_font_u, &cfg, &theme);
                drop(st);
                let text_w  = text.size().0;
                let cur_max = { *max_line_width_u.get() };
                if text_w > cur_max { *max_line_width_u.get_mut() = text_w; }
                if let Some(o) = cv.get_game_object_mut(&names_u.code_text) {
                    o.position.0 = code_x - hs; o.position.1 = text_top;
                    o.set_clip_origin(Some((ex + cfg.text_x, ey)));
                    o.set_clip_size(Some((code_w + hs, eh)));
                    o.set_drawable(Box::new(text));
                }
                if let Some(o) = cv.get_game_object_mut(&names_u.gutter) {
                    o.position.0 = ex; o.position.1 = text_top;
                    o.set_clip_origin(Some((ex, ey)));
                    o.set_clip_size(Some((cfg.gutter_w, eh)));
                    o.set_drawable(Box::new(gutter));
                }
            }
        }

        if let Some(o) = cv.get_game_object_mut(&names_u.code_text) {
            o.position.0 = code_x - hs;
            o.set_clip_origin(Some((ex + cfg.text_x, ey)));
            o.set_clip_size(Some((code_w + hs, eh)));
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.bg) {
            o.position = (ex, ey); o.size = (ew, eh);
            o.set_image(tint_overlay(4000.0, 4000.0, theme.editor_bg));
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.gutter_bg) {
            o.position = (ex, ey); o.size = (cfg.gutter_w, eh);
            o.set_image(tint_overlay(4000.0, 4000.0, theme.gutter_bg));
        }

        {
            let st = state_u.lock().unwrap();
            let sel = st.selection();

            for i in 0..SEL_OVERLAY_COUNT {
                let abs_row = slice_start + i;
                let name    = sel_overlay_name(&id_prefix_u, i);

                let Some(o) = cv.get_game_object_mut(&name) else { continue };

                let Some(((r1, c1), (r2, c2))) = sel else {
                    o.visible = false;
                    continue;
                };
                if abs_row >= total || abs_row < r1 || abs_row > r2 {
                    o.visible = false;
                    continue;
                }

                let line = &st.lines[abs_row];
                let start_col = if abs_row == r1 { c1.min(line.len()) } else { 0 };
                let end_col   = if abs_row == r2 { c2.min(line.len()) } else { line.len() };

                let x_start = (code_x + start_col as f32 * cw - hs)
                    .max(code_x)
                    .min(ex + ew);
                let x_end   = (code_x + end_col as f32 * cw - hs)
                    .max(code_x)
                    .min(ex + ew);
                let row_y   = text_top + (abs_row as f32 - slice_start as f32) * lh;

                let sel_w = (x_end - x_start).max(cw * 0.5);

                let need_resize = (o.size.0 - sel_w).abs() > 0.5;

                o.position = (x_start, row_y);
                o.size     = (sel_w, lh);
                o.set_clip_origin(Some((code_x, ey)));
                o.set_clip_size(Some((code_w, eh)));
                o.visible  = true;

                if need_resize {
                    o.set_image(tint_overlay(sel_w.max(1.0), lh.max(1.0),
                        quartz::Color(38, 79, 120, 180)));
                }
            }
        }

        let cursor_x = code_x + cur_col as f32 * cw - hs;
        let cursor_y = text_top + (cur_row as f32 - slice_start as f32) * lh;
        let in_view  = cursor_x >= code_x && cursor_x < ex + ew - RIGHT_PAD
                    && cursor_y >= code_y  && cursor_y < ey + eh - cfg.text_y;

        {
            let dt   = 1.0f32 / 60.0;
            let idle = { *idle_timer.get() } + dt;
            *idle_timer.get_mut() = idle;
            let should_blink = cfg.cursor_blink && idle >= BLINK_IDLE_SECS;
            if should_blink {
                let bt = { *blink_timer.get() } + dt;
                *blink_timer.get_mut() = bt;
                if bt >= BLINK_RATE_SECS {
                    *blink_timer.get_mut() = 0.0;
                    let vis = { *cursor_vis.get() };
                    *cursor_vis.get_mut() = !vis;
                }
            } else { *cursor_vis.get_mut() = true; }

            if let Some(o) = cv.get_game_object_mut(&names_u.cursor) {
                let (cur_w, cur_h, _) = cfg.cursor_size();
                if o.size != (cur_w, cur_h) {
                    o.size = (cur_w, cur_h);
                    o.set_image(prism::canvas::Image {
                        shape: prism::canvas::ShapeType::Rectangle(0.0, (cur_w, cur_h), 0.0),
                        image: cfg.cursor_style.build_image(cur_w, cur_h).into(),
                        color: None,
                    });
                }
                o.position.0 = cursor_x;
                o.position.1 = cursor_y;
                o.visible    = in_view && { *cursor_vis.get() };
            }
        }
    });
}