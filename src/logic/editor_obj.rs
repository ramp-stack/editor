use flowmango::{Canvas, GameObject};
use quartz::{tint_overlay, Color};
use ramp::prism;

use crate::components::highlight::{
    build_colored_text, build_gutter_slice, AsyncParser, ParseCache,
};
use crate::components::image_view;
use crate::components::language::{file_lang, FileMode};
use crate::components::minimap::{self, MINIMAP_LH, MINIMAP_PAD_X, MINIMAP_W};
use crate::components::scroll::ScrollAxis;
use crate::components::selection;
use crate::constants::RIGHT_PAD;
use crate::editor::Editor;
use crate::editor::ObjNames;
use crate::editor::{sel_overlay_name, SEL_OVERLAY_COUNT};

use std::sync::Arc;

pub const MINIMAP_LINE_POOL: usize = 400;
pub const SCROLLBAR_W: f32 = 12.0;

pub fn minimap_line_name(prefix: &str, i: usize) -> String {
    format!("{}_mm_{}", prefix, i)
}
pub fn minimap_bg_name(prefix: &str) -> String {
    format!("{}_minimap_bg", prefix)
}
pub fn scrollbar_track_name(prefix: &str) -> String {
    format!("{}_sb_track", prefix)
}
pub fn scrollbar_thumb_name(prefix: &str) -> String {
    format!("{}_sb_thumb", prefix)
}

pub fn outline_rect(w: f32, h: f32, col: quartz::Color, filled: bool) -> quartz::Image {
    use image::{Rgba, RgbaImage};
    let (pw, ph) = (w.ceil() as u32, h.ceil() as u32);
    let mut img = RgbaImage::new(pw.max(1), ph.max(1));
    let border = Rgba([col.0, col.1, col.2, col.3]);
    let fill = if filled {
        Rgba([col.0, col.1, col.2, 30])
    } else {
        Rgba([0, 0, 0, 0])
    };
    for y in 0..ph {
        for x in 0..pw {
            img.put_pixel(
                x,
                y,
                if x == 0 || y == 0 || x == pw - 1 || y == ph - 1 {
                    border
                } else {
                    fill
                },
            );
        }
    }
    quartz::Image {
        shape: quartz::ShapeType::Rectangle(0.0, (w, h), 0.0),
        image: Arc::new(img),
        color: None,
    }
}

pub fn solid_rect(w: f32, h: f32, col: quartz::Color) -> quartz::Image {
    use image::{Rgba, RgbaImage};
    let (pw, ph) = (w.ceil() as u32, h.ceil() as u32);
    let mut img = RgbaImage::new(pw.max(1), ph.max(1));
    let px = Rgba([col.0, col.1, col.2, col.3]);
    for y in 0..ph {
        for x in 0..pw {
            img.put_pixel(x, y, px);
        }
    }
    quartz::Image {
        shape: quartz::ShapeType::Rectangle(0.0, (w, h), 0.0),
        image: Arc::new(img),
        color: None,
    }
}

//I think this acts as the render loop's memory. It tracks what was last drawn
//so the loop can skip redundant work.
struct RenderState {
    last_path: String,
    last_version: u64,
    last_rendered_version: u64,
    slice_start: usize,
    slice_end: usize,
    gutter_start: usize,
    gutter_end: usize,
    gutter_row: usize,
    mm_first: usize,
    mm_last: usize,
    mm_texts: Vec<quartz::Text>,
    max_line_width: f32,
    thumb_h: f32,
    mm_hovered: bool,
    sb_dragging: bool,
    cached_color_map: Vec<Color>,
}

impl RenderState {
    fn new() -> Self {
        Self {
            last_path: String::new(),
            last_version: u64::MAX,
            last_rendered_version: u64::MAX,
            slice_start: usize::MAX,
            slice_end: 0,
            gutter_start: usize::MAX,
            gutter_end: 0,
            gutter_row: usize::MAX,
            mm_first: usize::MAX,
            mm_last: 0,
            mm_texts: Vec::new(),
            max_line_width: 0.0,
            thumb_h: 0.0,
            mm_hovered: false,
            sb_dragging: false,
            cached_color_map: Vec::new(),
        }
    }

    fn invalidate(&mut self) {
        self.last_rendered_version = u64::MAX;
        self.slice_start = usize::MAX;
        self.slice_end = 0;
        self.gutter_start = usize::MAX;
        self.gutter_end = 0;
        self.gutter_row = usize::MAX;
        self.mm_first = usize::MAX;
        self.mm_last = 0;
        self.mm_texts.clear();
        self.max_line_width = 0.0;
    }
}

struct RsSnapshot {
    last_path: String,
    last_version: u64,
    last_rendered_version: u64,
    slice_start: usize,
    slice_end: usize,
    gutter_start: usize,
    gutter_end: usize,
    gutter_row: usize,
    mm_first: usize,
    mm_last: usize,
    max_line_width: f32,
    thumb_h: f32,
    mm_hovered: bool,
    sb_dragging: bool,
    mm_texts_empty: bool,
    mm_texts_len: usize,
}

fn snapshot(rs: &quartz::Shared<RenderState>) -> RsSnapshot {
    let r = rs.get();
    RsSnapshot {
        last_path: r.last_path.clone(),
        last_version: r.last_version,
        last_rendered_version: r.last_rendered_version,
        slice_start: r.slice_start,
        slice_end: r.slice_end,
        gutter_start: r.gutter_start,
        gutter_end: r.gutter_end,
        gutter_row: r.gutter_row,
        mm_first: r.mm_first,
        mm_last: r.mm_last,
        max_line_width: r.max_line_width,
        thumb_h: r.thumb_h,
        mm_hovered: r.mm_hovered,
        sb_dragging: r.sb_dragging,
        mm_texts_empty: r.mm_texts.is_empty(),
        mm_texts_len: r.mm_texts.len(),
    }
}

pub fn setup(cv: &mut Canvas, ed: &Editor) {
    let names = ObjNames::from_prefix(&ed.id_prefix);
    let cfg = *ed.cfg.get();
    let theme = ed.theme.get().clone();
    let lh = cfg.line_height();
    let ex = *ed.live_x.get();
    let ey = *ed.live_y.get();
    let ew = *ed.live_w.get();
    let eh = *ed.live_h.get();

    let (init_mode, init_path) = {
        let st = ed.state.lock().unwrap();
        (st.mode.clone(), st.path.clone())
    };

    let code_x = ex + cfg.text_x;
    let code_y = ey + cfg.text_y;
    let code_w = ew - cfg.text_x - RIGHT_PAD - MINIMAP_W;
    let mmx = ex + ew - MINIMAP_W;
    let sb_x = ex + ew - SCROLLBAR_W;

    cv.add_game_object(
        names.bg.clone(),
        GameObject::build(&names.bg)
            .position(ex, ey)
            .size(ew, eh)
            .layer(0)
            .image(tint_overlay(
                4000.0,
                4000.0,
                theme.get("editor_bg").copied().unwrap(),
            ))
            .finish(),
    );

    image_view::mount(
        cv,
        &ed.id_prefix,
        &init_mode,
        &init_path,
        ex,
        ey,
        ew,
        eh,
        1,
        &ed.code_font,
    );

    let viewport_lines = if eh > cfg.text_y {
        ((eh - cfg.text_y) / lh).ceil() as usize + 2
    } else {
        2
    };
    let init_source = ed.state.lock().unwrap().lines.join("\n");
    let init_cache = ParseCache::build(init_source);

    let (init_text, init_gutter) = {
        let st = ed.state.lock().unwrap();
        let end = viewport_lines.min(st.lines.len().max(1));
        let lines = if st.lines.is_empty() {
            vec![String::new()]
        } else {
            st.lines[..end].to_vec()
        };
        (
            build_colored_text(
                &lines,
                &ed.code_font,
                &cfg,
                &theme,
                &file_lang(&init_path),
                &init_cache,
                0,
            ),
            build_gutter_slice(0, end.max(1), 0, &ed.gutter_font, &cfg, &theme),
        )
    };

    cv.add_game_object(names.gutter_bg.clone(), {
        let mut o = GameObject::build(&names.gutter_bg)
            .position(ex, ey)
            .size(cfg.gutter_w, eh)
            .layer(2)
            .image(tint_overlay(
                4000.0,
                4000.0,
                theme.get("gutter_bg").copied().unwrap(),
            ))
            .finish();
        o.visible = init_mode == FileMode::Text;
        o
    });

    {
        let mut o = GameObject::build(&names.gutter)
            .position(ex, code_y)
            .size(4000.0, 4000.0)
            .layer(3)
            .clip()
            .clip_origin(ex, ey)
            .clip_size(cfg.gutter_w, eh)
            .finish();
        o.set_drawable(Box::new(init_gutter));
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(names.gutter.clone(), o);
    }

    for i in 0..SEL_OVERLAY_COUNT {
        let name = sel_overlay_name(&ed.id_prefix, i);
        let mut o = GameObject::build(&name)
            .position(code_x, code_y + i as f32 * lh)
            .size(1.0, lh)
            .layer(2)
            .image(tint_overlay(
                1.0,
                lh.max(1.0),
                quartz::Color(38, 79, 120, 180),
            ))
            .clip()
            .clip_origin(code_x, ey)
            .clip_size(code_w, eh)
            .finish();
        o.visible = false;
        cv.add_game_object(name, o);
    }

    {
        let mut o = GameObject::build(&names.code_text)
            .position(code_x, code_y)
            .size(4000.0, 4000.0)
            .layer(1)
            .clip()
            .clip_origin(ex + cfg.text_x, ey)
            .clip_size(code_w, eh)
            .finish();
        o.set_drawable(Box::new(init_text));
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(names.code_text.clone(), o);
    }

    {
        let (cur_w, cur_h, _) = cfg.cursor_size();
        let mut o = GameObject::build(&names.cursor)
            .position(code_x, code_y)
            .size(cur_w, cur_h)
            .layer(4)
            .pivot(0.0, 0.0)
            .image(prism::canvas::Image {
                shape: prism::canvas::ShapeType::Rectangle(0.0, (cur_w, cur_h), 0.0),
                image: cfg.cursor_style.build_image(cur_w, cur_h).into(),
                color: None,
            })
            .finish();
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(names.cursor.clone(), o);
    }

    {
        let bg_c = theme
            .get("gutter_bg")
            .copied()
            .unwrap_or(quartz::Color(13, 14, 20, 255));
        let bname = minimap_bg_name(&ed.id_prefix);
        let mut o = GameObject::build(&bname)
            .position(mmx, ey)
            .size(MINIMAP_W, eh)
            .layer(2)
            .image(tint_overlay(MINIMAP_W, eh, bg_c))
            .finish();
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(bname, o);
    }

    {
        let vname = format!("{}_mm_viewport", &ed.id_prefix);
        let mut o = GameObject::build(&vname)
            .position(mmx, ey)
            .size(MINIMAP_W, 20.0)
            .layer(5)
            .finish();
        o.set_image(outline_rect(
            MINIMAP_W,
            20.0,
            quartz::Color(150, 170, 220, 120),
            false,
        ));
        o.visible = false;
        cv.add_game_object(vname, o);
    }

    for i in 0..MINIMAP_LINE_POOL {
        let lname = minimap_line_name(&ed.id_prefix, i);
        let mut o = GameObject::build(&lname)
            .position(mmx + MINIMAP_PAD_X, ey + i as f32 * MINIMAP_LH)
            .size(MINIMAP_W - MINIMAP_PAD_X, MINIMAP_LH)
            .layer(3)
            .clip()
            .clip_origin(mmx, ey)
            .clip_size(MINIMAP_W, eh)
            .finish();
        o.visible = false;
        cv.add_game_object(lname, o);
    }

    {
        let tname = scrollbar_track_name(&ed.id_prefix);
        let mut o = GameObject::build(&tname)
            .position(sb_x, ey)
            .size(SCROLLBAR_W, eh)
            .layer(6)
            .image(solid_rect(SCROLLBAR_W, eh, quartz::Color(30, 31, 40, 180)))
            .finish();
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(tname, o);
    }

    {
        let thname = scrollbar_thumb_name(&ed.id_prefix);
        let mut o = GameObject::build(&thname)
            .position(sb_x, ey)
            .size(SCROLLBAR_W, 40.0)
            .layer(7)
            .image(solid_rect(
                SCROLLBAR_W,
                40.0,
                quartz::Color(80, 90, 120, 200),
            ))
            .finish();
        o.visible = false;
        cv.add_game_object(thname, o);
    }
}

pub fn register(cv: &mut Canvas, ed: &Editor) {
    let cfg_u = ed.cfg.clone();
    let theme_u = ed.theme.clone();
    let code_font_u = ed.code_font.clone();
    let gutter_font_u = ed.gutter_font.clone();
    let v_scroll_u = ed.v_scroll.clone();
    let h_scroll_u = ed.h_scroll.clone();
    let state_u = ed.state.clone();
    let blink_timer_u = ed.blink_timer.clone();
    let idle_timer_u = ed.idle_timer.clone();
    let cursor_vis_u = ed.cursor_vis.clone();
    let autosave_timer_u = ed.autosave_timer.clone();
    let live_x_u = ed.live_x.clone();
    let live_y_u = ed.live_y.clone();
    let live_w_u = ed.live_w.clone();
    let live_h_u = ed.live_h.clone();
    let names_u = ObjNames::from_prefix(&ed.id_prefix);
    let id_prefix_u = ed.id_prefix.clone();
    let img_loaded_key_u = ed.img_loaded_key.clone();
    let scroll_intent_u = ed.scroll_intent.clone();
    let dragging_u = ed.dragging.clone();
    let sb_dragging_u = ed.sb_dragging.clone();

    {
        let v_scroll = ed.v_scroll.clone();
        let live_x = ed.live_x.clone();
        let live_y = ed.live_y.clone();
        let live_w = ed.live_w.clone();
        let live_h = ed.live_h.clone();
        let state = ed.state.clone();
        let cfg = ed.cfg.clone();

        cv.on_mouse_press(move |_cv, _btn, (mx, my)| {
            let ex = *live_x.get();
            let ey = *live_y.get();
            let ew = *live_w.get();
            let eh = *live_h.get();
            let mmx = ex + ew - MINIMAP_W;
            let sb_x = ex + ew - SCROLLBAR_W;
            if mx < mmx || mx >= sb_x || my < ey || my > ey + eh {
                return;
            }

            let cfg = *cfg.get();
            let lh = cfg.line_height();
            let total = state.lock().unwrap().lines.len().max(1);
            let viewport_h = (eh - cfg.text_y).max(0.0);
            let v_max = (total as f32 * lh - viewport_h).max(0.0);
            let gs = v_scroll.get().offset;

            let total_mm_h = total as f32 * MINIMAP_LH;
            let max_mm_off = (total_mm_h - eh).max(0.0);
            let mm_scroll_px = if v_max > 0.0 {
                (gs / v_max) * max_mm_off
            } else {
                0.0
            };
            let click_mm_px = mm_scroll_px + (my - ey);
            let rect_h_mm = (viewport_h / lh) * MINIMAP_LH;
            let rect_top_mm = (click_mm_px - rect_h_mm * 0.5)
                .max(0.0)
                .min((total_mm_h - rect_h_mm).max(0.0));
            let target_off = (rect_top_mm / MINIMAP_LH * lh).max(0.0).min(v_max);
            v_scroll.get_mut().jump_to(target_off, v_max);
        });
    }

    let rs = quartz::Shared::new(RenderState::new());
    let parser = Arc::new(AsyncParser::new());

    cv.on_update(move |cv| {
        let frame_start = std::time::Instant::now();
        let cfg = *cfg_u.get();
        let theme = theme_u.get().clone();
        let lh = cfg.line_height();
        let cw = cfg.char_width();
        let ex = *live_x_u.get();
        let ey = *live_y_u.get();
        let ew = *live_w_u.get();
        let eh = *live_h_u.get();
        let code_x = ex + cfg.text_x;
        let code_y = ey + cfg.text_y;
        let code_w = ew - cfg.text_x - RIGHT_PAD - MINIMAP_W;
        let mmx = ex + ew - MINIMAP_W;
        let sb_x = ex + ew - SCROLLBAR_W;

        let (cur_mode, cur_path, cur_lang, total, cur_row, cur_col, content_version) = {
            let st = state_u.lock().unwrap();
            (
                st.mode.clone(),
                st.path.clone(),
                st.lang.clone(),
                st.lines.len(),
                st.cursor_row,
                st.cursor_col,
                st.content_version,
            )
        };
        let is_image = cur_mode == FileMode::Image;

        if let Some(o) = cv.get_game_object_mut(&names_u.bg) {
            o.position = (ex, ey);
            o.size = (ew, eh);
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.gutter_bg) {
            o.position = (ex, ey);
            o.size = (cfg.gutter_w, eh);
        }

        if is_image {
            image_view::update(
                cv,
                &id_prefix_u,
                &cur_path,
                ex,
                ey,
                ew,
                eh,
                &img_loaded_key_u,
                &code_font_u,
            );
            image_view::show(
                cv,
                &id_prefix_u,
                &[
                    names_u.gutter_bg.as_str(),
                    names_u.gutter.as_str(),
                    names_u.code_text.as_str(),
                    names_u.cursor.as_str(),
                ],
            );
            for i in 0..SEL_OVERLAY_COUNT {
                if let Some(o) = cv.get_game_object_mut(&sel_overlay_name(&id_prefix_u, i)) {
                    o.visible = false;
                }
            }
            if let Some(o) = cv.get_game_object_mut(&scrollbar_track_name(&id_prefix_u)) {
                o.visible = false;
            }
            if let Some(o) = cv.get_game_object_mut(&scrollbar_thumb_name(&id_prefix_u)) {
                o.visible = false;
            }
            for i in 0..MINIMAP_LINE_POOL {
                if let Some(o) = cv.get_game_object_mut(&minimap_line_name(&id_prefix_u, i)) {
                    o.visible = false;
                }
            }
            if let Some(o) = cv.get_game_object_mut(&minimap_bg_name(&id_prefix_u)) {
                o.visible = false;
            }
            return;
        }

        image_view::hide(
            cv,
            &id_prefix_u,
            &[
                names_u.gutter_bg.as_str(),
                names_u.gutter.as_str(),
                names_u.code_text.as_str(),
                names_u.cursor.as_str(),
            ],
        );

        if total == 0 {
            return;
        }

        {
            let t = *autosave_timer_u.get() + 1.0 / 60.0;
            *autosave_timer_u.get_mut() = t;
            if t >= crate::constants::AUTOSAVE_INTERVAL_SECS {
                *autosave_timer_u.get_mut() = 0.0;
                let mut st = state_u.lock().unwrap();
                if st.dirty {
                    st.save();
                }
            }
        }

        let mut sn = snapshot(&rs);

        let path_changed = sn.last_path != cur_path;
        let version_changed = sn.last_version != content_version;
        if path_changed || version_changed {
            let src = state_u.lock().unwrap().lines.join("\n");
            parser.request(content_version, src);
            let mut r = rs.get_mut();
            r.last_path = cur_path.clone();
            r.last_version = content_version;
            r.invalidate();
            drop(r);
            sn = snapshot(&rs);
        }

        let ready_ver = parser.ready_version();
        let new_parse_ready = ready_ver != u64::MAX
            && ready_ver != sn.last_rendered_version
            && ready_ver == content_version;

        let waiting_for_parse = ready_ver != content_version;

        if new_parse_ready {
            let cache = parser.borrow();
            if let Some(cache) = &*cache {
                rs.get_mut().cached_color_map = minimap::build_color_map(cache, &theme);
            }
        }

        if sn.max_line_width <= 0.0 {
            let st = state_u.lock().unwrap();
            if let Some(longest) = st.lines.iter().max_by_key(|l| l.chars().count()) {
                rs.get_mut().max_line_width = longest.chars().count() as f32 * cw;
            }
        }

        let intent = *scroll_intent_u.get();
        if intent != 0.0 {
            v_scroll_u.get_mut().set_intent(intent);
        }

        let viewport_h = (eh - cfg.text_y).max(0.0);
        let v_max = (total as f32 * lh - viewport_h).max(0.0);
        v_scroll_u.get_mut().tick(cfg.scroll_friction, v_max);

        let h_max = (sn.max_line_width - code_w).max(0.0);
        h_scroll_u.get_mut().tick(cfg.scroll_friction, h_max);
        if h_scroll_u.get().offset > h_max {
            h_scroll_u.get_mut().offset = h_max;
        }

        let gs = v_scroll_u.get().offset;
        let hs = h_scroll_u.get().offset;

        let first_visible = (gs / lh).floor() as usize;
        let sub_line_offset = gs - first_visible as f32 * lh;
        let (slice_start, text_top) = if first_visible > 0 {
            (first_visible - 1, code_y - sub_line_offset - lh)
        } else {
            (0, code_y - sub_line_offset)
        };
        let visible_lines = if eh > cfg.text_y {
            ((eh - cfg.text_y) / lh).ceil() as usize + 2
        } else {
            2
        };
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

        let slice_moved = sn.slice_start != slice_start || sn.slice_end != last_visible;

        let scroll_vel = v_scroll_u.get().velocity.abs();
        let fast_scrolling = scroll_vel > 8.0 * lh;
        if (new_parse_ready || (slice_moved && !fast_scrolling)) && last_visible > slice_start {
            let cache = parser.borrow();
            let st = state_u.lock().unwrap();
            let text = build_colored_text(
                &st.lines[slice_start..last_visible],
                &code_font_u,
                &cfg,
                &theme,
                &cur_lang,
                &cache,
                slice_start,
            );
            drop(st);
            drop(cache);
            if let Some(o) = cv.get_game_object_mut(&names_u.code_text) {
                o.set_drawable(Box::new(text));
            }
            let mut r = rs.get_mut();
            r.slice_start = slice_start;
            r.slice_end = last_visible;
            r.last_rendered_version = ready_ver;
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.code_text) {
            o.position = (code_x - hs, text_top);
            o.set_clip_origin(Some((ex, ey)));
            o.set_clip_size(Some((ew - MINIMAP_W, eh)));
            o.visible = true;
        }

        let gutter_moved = sn.gutter_start != slice_start
            || sn.gutter_end != last_visible
            || sn.gutter_row != cur_row;
        if (new_parse_ready || gutter_moved) && last_visible > slice_start {
            let gutter = build_gutter_slice(
                slice_start,
                last_visible,
                cur_row,
                &gutter_font_u,
                &cfg,
                &theme,
            );
            if let Some(o) = cv.get_game_object_mut(&names_u.gutter) {
                o.set_drawable(Box::new(gutter));
            }
            let mut r = rs.get_mut();
            r.gutter_start = slice_start;
            r.gutter_end = last_visible;
            r.gutter_row = cur_row;
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.gutter) {
            o.position = (ex, text_top);
            o.set_clip_origin(Some((ex, ey)));
            o.set_clip_size(Some((cfg.gutter_w, eh)));
            o.visible = true;
        }
        if let Some(o) = cv.get_game_object_mut(&names_u.gutter_bg) {
            o.visible = true;
        }

        {
            let st = state_u.lock().unwrap();
            let overlays = selection::overlay_rects(
                st.sel.anchor,
                st.sel.active,
                &st.lines,
                slice_start,
                visible_lines,
                text_top,
                code_x,
                hs,
                cw,
                lh,
                cw * 0.5,
            );
            for i in 0..SEL_OVERLAY_COUNT {
                let name = sel_overlay_name(&id_prefix_u, i);
                let Some(o) = cv.get_game_object_mut(&name) else {
                    continue;
                };
                match overlays.iter().find(|v| v.abs_row == slice_start + i) {
                    None => {
                        o.visible = false;
                    }
                    Some(ovr) => {
                        if (o.size.0 - ovr.width).abs() > 0.5 {
                            o.set_image(tint_overlay(
                                ovr.width.max(1.0),
                                lh.max(1.0),
                                quartz::Color(38, 79, 120, 180),
                            ));
                        }
                        o.position = (ovr.x_start, ovr.y);
                        o.size = (ovr.width, lh);
                        o.set_clip_origin(Some((code_x, ey)));
                        o.set_clip_size(Some((code_w, eh)));
                        o.visible = true;
                    }
                }
            }
        }

        {
            let dt = 1.0f32 / 60.0;
            let idle = *idle_timer_u.get() + dt;
            *idle_timer_u.get_mut() = idle;
            if cfg.cursor_blink && idle >= crate::constants::BLINK_IDLE_SECS {
                let bt = *blink_timer_u.get() + dt;
                *blink_timer_u.get_mut() = bt;
                if bt >= crate::constants::BLINK_RATE_SECS {
                    *blink_timer_u.get_mut() = 0.0;
                    let vis = *cursor_vis_u.get();
                    *cursor_vis_u.get_mut() = !vis;
                }
            } else {
                *cursor_vis_u.get_mut() = true;
            }

            let cursor_x = code_x - hs + cur_col as f32 * cw;
            let cursor_y = text_top + (cur_row as f32 - slice_start as f32) * lh;
            let in_view = cursor_x >= code_x - 1.0
                && cursor_x < ex + ew - MINIMAP_W - RIGHT_PAD
                && cursor_y >= ey
                && cursor_y < ey + eh - cfg.text_y;

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
                o.position = (cursor_x, cursor_y);
                o.visible = in_view && *cursor_vis_u.get();
            }
        }

        {
            let thumb_h = if v_max > 0.0 {
                (viewport_h / (viewport_h + v_max) * eh).max(20.0).min(eh)
            } else {
                eh
            };
            let thumb_top = if v_max > 0.0 {
                (gs / v_max) * (eh - thumb_h)
            } else {
                0.0
            };

            if let Some(o) = cv.get_game_object_mut(&scrollbar_track_name(&id_prefix_u)) {
                o.position = (sb_x, ey);
                o.size = (SCROLLBAR_W, eh);
                o.visible = true;
            }

            let needs_resize = (sn.thumb_h - thumb_h).abs() > 0.5;
            if let Some(o) = cv.get_game_object_mut(&scrollbar_thumb_name(&id_prefix_u)) {
                if needs_resize {
                    o.set_image(solid_rect(
                        SCROLLBAR_W,
                        thumb_h,
                        quartz::Color(100, 110, 150, 220),
                    ));
                    o.size = (SCROLLBAR_W, thumb_h);
                }
                o.position = (sb_x, ey + thumb_top);
                o.visible = v_max > 0.0;
            }
            if needs_resize {
                rs.get_mut().thumb_h = thumb_h;
            }
        }

        {
            let total_mm_h = total as f32 * MINIMAP_LH;
            let max_mm_off = (total_mm_h - eh).max(0.0);
            let mm_scroll_px = if v_max > 0.0 {
                (gs / v_max) * max_mm_off
            } else {
                0.0
            };
            let first_mm = (mm_scroll_px / MINIMAP_LH).floor() as usize;
            let sub_off = mm_scroll_px - first_mm as f32 * MINIMAP_LH;
            let visible_mm = ((eh / MINIMAP_LH).ceil() as usize + 2).min(MINIMAP_LINE_POOL);
            let last_mm = (first_mm + visible_mm).min(total);

            let mm_moved = sn.mm_first != first_mm || sn.mm_last != last_mm;

            if new_parse_ready
                || mm_moved
                || sn.mm_texts_empty && !rs.get().cached_color_map.is_empty()
            {
                let st = state_u.lock().unwrap();
                let built = minimap::build_minimap_texts(
                    &st.lines[first_mm..last_mm],
                    &rs.get().cached_color_map,
                    &code_font_u,
                );
                drop(st);
                let mut r = rs.get_mut();
                r.mm_texts = built;
                r.mm_first = first_mm;
                r.mm_last = last_mm;
            }

            let mm_texts_len = rs.get().mm_texts.len();

            for slot in 0..MINIMAP_LINE_POOL {
                let lname = minimap_line_name(&id_prefix_u, slot);
                let Some(o) = cv.get_game_object_mut(&lname) else {
                    continue;
                };
                if slot >= visible_mm || slot >= mm_texts_len {
                    o.visible = false;
                    continue;
                }
                let maybe_text = if new_parse_ready || mm_moved {
                    Some(rs.get().mm_texts[slot].clone())
                } else {
                    None
                };
                o.position = (mmx + MINIMAP_PAD_X, ey + slot as f32 * MINIMAP_LH - sub_off);
                o.set_clip_origin(Some((mmx, ey)));
                o.set_clip_size(Some((MINIMAP_W, eh)));
                if let Some(text) = maybe_text {
                    o.set_drawable(Box::new(text));
                }
                o.visible = true;
            }

            if let Some(o) = cv.get_game_object_mut(&minimap_bg_name(&id_prefix_u)) {
                o.position = (mmx, ey);
                o.size = (MINIMAP_W, eh);
                o.visible = true;
            }

            let vname = format!("{}_mm_viewport", &*id_prefix_u);
            let mouse_pos = cv.mouse_position();
            let is_sb_drag = *sb_dragging_u.get();

            if let Some(o) = cv.get_game_object_mut(&vname) {
                let rect_h = ((viewport_h / lh) * MINIMAP_LH).max(4.0);
                let rect_top_mm = (gs / lh) * MINIMAP_LH;
                let rect_y = (ey + rect_top_mm - mm_scroll_px)
                    .max(ey)
                    .min(ey + eh - rect_h);
                let hovered = !is_sb_drag
                    && mouse_pos
                        .map(|(mx, my)| mx >= mmx && mx < sb_x && my >= ey && my <= ey + eh)
                        .unwrap_or(false);

                let size_changed = (o.size.1 - rect_h).abs() > 0.5;
                let hover_changed = sn.mm_hovered != hovered;
                let drag_changed = sn.sb_dragging != is_sb_drag;

                if size_changed || hover_changed || drag_changed {
                    o.set_image(outline_rect(
                        MINIMAP_W,
                        rect_h,
                        quartz::Color(150, 170, 220, 120),
                        hovered,
                    ));
                    o.size = (MINIMAP_W, rect_h);
                    let mut r = rs.get_mut();
                    r.mm_hovered = hovered;
                    r.sb_dragging = is_sb_drag;
                }
                o.position = (mmx, rect_y);
                o.visible = true;
            }
        }
        println!("frame: {:?}", frame_start.elapsed());
    });
}
