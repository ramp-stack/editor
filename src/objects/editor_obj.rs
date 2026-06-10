use flowmango::{Canvas, GameObject};
use quartz::tint_overlay;
use ramp::prism;

use crate::components::highlight::{build_colored_text, build_gutter_slice, ParseCache};
use crate::components::image_view;
use crate::components::language::{file_lang, FileMode};
use crate::components::minimap::{self, MINIMAP_LH, MINIMAP_PAD_X, MINIMAP_W};
use crate::constants::RIGHT_PAD;
use crate::editor::Editor;
use crate::editor::ObjNames;
use crate::editor::{sel_overlay_name, SEL_OVERLAY_COUNT};

pub const MINIMAP_LINE_POOL: usize = 400;
pub const SCROLLBAR_W: f32 = 12.0;
pub const SCROLLBAR_MIN_THUMB_H: f32 = 40.0;

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
    use std::sync::Arc as StdArc;
    let pw = w.ceil() as u32;
    let ph = h.ceil() as u32;
    let mut img = RgbaImage::new(pw.max(1), ph.max(1));
    let border = Rgba([col.0, col.1, col.2, col.3]);
    let fill = if filled {
        Rgba([col.0, col.1, col.2, 30])
    } else {
        Rgba([0, 0, 0, 0])
    };
    for y in 0..ph {
        for x in 0..pw {
            let on_edge = x == 0 || y == 0 || x == pw - 1 || y == ph - 1;
            img.put_pixel(x, y, if on_edge { border } else { fill });
        }
    }
    quartz::Image {
        shape: quartz::ShapeType::Rectangle(0.0, (w, h), 0.0),
        image: StdArc::new(img),
        color: None,
    }
}

pub fn solid_rect(w: f32, h: f32, col: quartz::Color) -> quartz::Image {
    use image::{Rgba, RgbaImage};
    use std::sync::Arc as StdArc;
    let pw = w.ceil() as u32;
    let ph = h.ceil() as u32;
    let mut img = RgbaImage::new(pw.max(1), ph.max(1));
    let px = Rgba([col.0, col.1, col.2, col.3]);
    for y in 0..ph {
        for x in 0..pw {
            img.put_pixel(x, y, px);
        }
    }
    quartz::Image {
        shape: quartz::ShapeType::Rectangle(0.0, (w, h), 0.0),
        image: StdArc::new(img),
        color: None,
    }
}

pub fn setup(cv: &mut Canvas, ed: &Editor) {
    let names = ObjNames::from_prefix(&ed.id_prefix);
    let cfg = *ed.cfg.get();
    let theme = ed.theme.get().clone();
    let lh = cfg.line_height();
    let ex = { *ed.live_x.get() };
    let ey = { *ed.live_y.get() };
    let ew = { *ed.live_w.get() };
    let eh = { *ed.live_h.get() };

    let (init_mode, init_path) = {
        let st = ed.state.lock().unwrap();
        (st.mode.clone(), st.path.clone())
    };

    let code_x = ex + cfg.text_x;
    let code_y = ey + cfg.text_y;
    let code_w = ew - cfg.text_x - RIGHT_PAD - MINIMAP_W;
    let mx = ex + ew - MINIMAP_W;
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

    let init_source = {
        let st = ed.state.lock().unwrap();
        st.lines.join("\n")
    };
    let init_cache = ParseCache::build(init_source);
    let init_color_map = init_cache
        .as_ref()
        .map(|c| minimap::build_color_map(c, &theme))
        .unwrap_or_default();

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
                &init_color_map,
                0,
            ),
            build_gutter_slice(0, end.max(1), 0, &ed.gutter_font, &cfg, &theme),
        )
    };

    if init_mode == FileMode::Text {
        let st = ed.state.lock().unwrap();
        if let Some(line) = st.lines.iter().max_by_key(|l| l.chars().count()) {
            let t = build_colored_text(
                std::slice::from_ref(line),
                &ed.code_font,
                &cfg,
                &theme,
                &file_lang(&init_path),
                &init_color_map,
                0,
            );
            *ed.max_line_width.get_mut() = t.size().0;
        }
    }

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
            .position(mx, ey)
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
            .position(mx, ey)
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
            .position(mx + MINIMAP_PAD_X, ey + i as f32 * MINIMAP_LH)
            .size(MINIMAP_W - MINIMAP_PAD_X, MINIMAP_LH)
            .layer(3)
            .clip()
            .clip_origin(mx, ey)
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
            .size(SCROLLBAR_W, SCROLLBAR_MIN_THUMB_H)
            .layer(7)
            .image(solid_rect(
                SCROLLBAR_W,
                SCROLLBAR_MIN_THUMB_H,
                quartz::Color(80, 90, 120, 200),
            ))
            .finish();
        o.visible = false;
        cv.add_game_object(thname, o);
    }
}
