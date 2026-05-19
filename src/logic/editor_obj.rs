// logic/editor_obj.rs — Per-frame update logic (on_update).

use flowmango::Canvas;
use quartz::tint_overlay;
use ramp::prism;

use crate::constants::*;
use crate::components::cursor::{cursor_in_view, cursor_position};
use crate::components::highlight::{build_gutter_slice, build_colored_text};
use crate::components::image_view;
use crate::components::language::FileMode;
use crate::components::scroll::ScrollAxis;
use crate::components::selection;
use crate::editor::Editor;
use crate::editor::ObjNames;
use crate::editor::{sel_overlay_name, SEL_OVERLAY_COUNT};

pub fn register(cv: &mut Canvas, ed: &Editor) {
    let cfg_u            = ed.cfg.clone();
    let theme_u          = ed.theme.clone();
    let code_font_u      = ed.code_font.clone();
    let gutter_font_u    = ed.gutter_font.clone();
    let v_scroll_u       = ed.v_scroll.clone();
    let h_scroll_u       = ed.h_scroll.clone();
    let max_line_width_u = ed.max_line_width.clone();
    let state_u          = ed.state.clone();
    let blink_timer_u    = ed.blink_timer.clone();
    let idle_timer_u     = ed.idle_timer.clone();
    let cursor_vis_u     = ed.cursor_vis.clone();
    let autosave_timer_u = ed.autosave_timer.clone();
    let live_x_u         = ed.live_x.clone();
    let live_y_u         = ed.live_y.clone();
    let live_w_u         = ed.live_w.clone();
    let live_h_u         = ed.live_h.clone();
    let names_u          = ObjNames::from_prefix(&ed.id_prefix);
    let id_prefix_u      = ed.id_prefix.clone();
    let img_loaded_key_u = ed.img_loaded_key.clone();
    let scroll_intent_u  = ed.scroll_intent.clone();
    let dragging_u       = ed.dragging.clone();

    cv.on_update(move |cv| {
        let cfg    = *cfg_u.get();
        let theme  = theme_u.get().clone();
        let lh     = cfg.line_height();
        let cw     = cfg.char_width();
        let ex     = { *live_x_u.get() };
        let ey     = { *live_y_u.get() };
        let ew     = { *live_w_u.get() };
        let eh     = { *live_h_u.get() };
        let code_x = ex + cfg.text_x;
        let code_w = ew - cfg.text_x - RIGHT_PAD;
        let code_y = ey + cfg.text_y;

        let (cur_mode, cur_path, cur_lang) = {
            let st = state_u.lock().unwrap();
            (st.mode.clone(), st.path.clone(), st.lang.clone())
        };
        let is_image = cur_mode == FileMode::Image;

        if let Some(o) = cv.get_game_object_mut(&names_u.bg) {
            o.position = (ex, ey);
            o.size     = (ew, eh);
            o.set_image(tint_overlay(4000.0, 4000.0,
                theme.get("editor_bg").copied().unwrap()));
        }

        if is_image {
            image_view::update(cv, &id_prefix_u, &cur_path, ex, ey, ew, eh, &img_loaded_key_u);
            image_view::show(cv, &id_prefix_u, &[
                names_u.gutter_bg.as_str(), names_u.gutter.as_str(),
                names_u.code_text.as_str(), names_u.cursor.as_str(),
            ]);
            for i in 0..SEL_OVERLAY_COUNT {
                if let Some(o) = cv.get_game_object_mut(&sel_overlay_name(&id_prefix_u, i)) {
                    o.visible = false;
                }
            }
            return;
        }

        image_view::hide(cv, &id_prefix_u, &[
            names_u.gutter_bg.as_str(), names_u.gutter.as_str(),
            names_u.code_text.as_str(), names_u.cursor.as_str(),
        ]);

        {
            let timer = { *autosave_timer_u.get() } + 1.0 / 60.0;
            *autosave_timer_u.get_mut() = timer;
            if timer >= AUTOSAVE_INTERVAL_SECS {
                *autosave_timer_u.get_mut() = 0.0;
                let mut st = state_u.lock().unwrap();
                if st.dirty { st.save(); }
            }
        }

        let (total, cur_row, cur_col) = {
            let st = state_u.lock().unwrap();
            (st.lines.len(), st.cursor_row, st.cursor_col)
        };
        if total == 0 { return; }

        if { *max_line_width_u.get() } <= 0.0 {
            let st = state_u.lock().unwrap();
            if let Some(longest) = st.lines.iter().max_by_key(|l| l.chars().count()) {
                let t = build_colored_text(
                    std::slice::from_ref(longest), &code_font_u, &cfg, &theme, &cur_lang,
                );
                *max_line_width_u.get_mut() = t.size().0;
            }
        }

        let intent = { *scroll_intent_u.get() };
        if intent != 0.0 {
            v_scroll_u.get_mut().set_intent(intent);
        }

        let viewport_h = (eh - cfg.text_y).max(0.0);
        let v_max      = (total as f32 * lh - viewport_h).max(0.0);
        v_scroll_u.get_mut().tick(cfg.scroll_friction, v_max);

        let h_max = ({ *max_line_width_u.get() } - code_w).max(0.0);
        h_scroll_u.get_mut().tick(cfg.scroll_friction, h_max);
        {
            let cur = h_scroll_u.get().offset;
            if cur > h_max { h_scroll_u.get_mut().offset = h_max; }
        }

        let gs = v_scroll_u.get().offset;
        let hs = h_scroll_u.get().offset;

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

        if *dragging_u.get() {
            if let Some((mx, my)) = cv.mouse_position() {
                let mx_c = mx.clamp(ex, ex + ew);
                let my_c = my.clamp(ey, ey + eh);
                let mut st = state_u.lock().unwrap();
                if !st.lines.is_empty() {
                    let row = (((my_c - ey - cfg.text_y + gs) / lh).floor().max(0.0) as usize)
                        .min(st.lines.len().saturating_sub(1));
                    let col = (((mx_c - ex - cfg.text_x + hs) / cw).floor().max(0.0) as usize)
                        .min(selection::char_len(&st.lines[row]));
                    st.cursor_row = row;
                    st.cursor_col = col;
                    st.sel.extend((row, col));
                }
            }
        }

        {
            let st = state_u.lock().unwrap();
            if last_visible > slice_start {
                let text = build_colored_text(
                    &st.lines[slice_start..last_visible], &code_font_u, &cfg, &theme, &cur_lang,
                );
                let gutter = build_gutter_slice(
                    slice_start, last_visible, cur_row, &gutter_font_u, &cfg, &theme,
                );
                drop(st);
                if let Some(o) = cv.get_game_object_mut(&names_u.code_text) {
                    o.position.0 = code_x - hs;
                    o.position.1 = text_top;
                    o.set_clip_origin(Some((ex + cfg.text_x, ey)));
                    o.set_clip_size(Some((code_w + hs, eh)));
                    o.set_drawable(Box::new(text));
                }
                if let Some(o) = cv.get_game_object_mut(&names_u.gutter) {
                    o.position.0 = ex;
                    o.position.1 = text_top;
                    o.set_clip_origin(Some((ex, ey)));
                    o.set_clip_size(Some((cfg.gutter_w, eh)));
                    o.set_drawable(Box::new(gutter));
                }
            }
        }

        if let Some(o) = cv.get_game_object_mut(&names_u.code_text) {
            o.position.0 = code_x - hs;
            o.set_clip_origin(Some((ex, ey)));
            o.set_clip_size(Some((ew, eh)));
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.bg) {
            o.position = (ex, ey);
            o.size     = (ew, eh);
            o.set_image(tint_overlay(4000.0, 4000.0,
                theme.get("editor_bg").copied().unwrap()));
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.gutter_bg) {
            o.position = (ex, ey);
            o.size     = (cfg.gutter_w, eh);
            o.set_image(tint_overlay(4000.0, 4000.0,
                theme.get("gutter_bg").copied().unwrap()));
        }

        {
            let st       = state_u.lock().unwrap();
            let overlays = selection::overlay_rects(
                st.sel.anchor, st.sel.active, &st.lines,
                slice_start, visible_lines,
                text_top, code_x, hs, cw, lh, cw * 0.5,
            );

            for i in 0..SEL_OVERLAY_COUNT {
                let name      = sel_overlay_name(&id_prefix_u, i);
                let Some(o)   = cv.get_game_object_mut(&name) else { continue };
                let Some(ovr) = overlays.iter().find(|v| v.abs_row == slice_start + i) else {
                    o.visible = false;
                    continue;
                };

                let need_resize = (o.size.0 - ovr.width).abs() > 0.5;
                o.position      = (ovr.x_start, ovr.y);
                o.size          = (ovr.width, lh);
                o.set_clip_origin(Some((code_x, ey)));
                o.set_clip_size(Some((code_w, eh)));
                o.visible       = true;

                if need_resize {
                    o.set_image(tint_overlay(
                        ovr.width.max(1.0), lh.max(1.0),
                        quartz::Color(38, 79, 120, 180),
                    ));
                }
            }
        }

        let (cursor_x, cursor_y) = cursor_position(
            cur_row, cur_col, slice_start, text_top, code_x, hs, cw, lh,
        );
        let in_view = cursor_in_view(cursor_x, cursor_y, code_x, ex, ey, ew, eh, RIGHT_PAD, cfg.text_y);

        {
            let dt   = 1.0f32 / 60.0;
            let idle = { *idle_timer_u.get() } + dt;
            *idle_timer_u.get_mut() = idle;

            let should_blink = cfg.cursor_blink && idle >= BLINK_IDLE_SECS;
            if should_blink {
                let bt = { *blink_timer_u.get() } + dt;
                *blink_timer_u.get_mut() = bt;
                if bt >= BLINK_RATE_SECS {
                    *blink_timer_u.get_mut() = 0.0;
                    let vis = { *cursor_vis_u.get() };
                    *cursor_vis_u.get_mut() = !vis;
                }
            } else {
                *cursor_vis_u.get_mut() = true;
            }

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
                o.visible    = in_view && { *cursor_vis_u.get() };
            }
        }
    });
}