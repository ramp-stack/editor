use quartz::tint_overlay;
use ramp::prism;
use flowmango::Canvas;
use flowmango::GameObject;

use crate::constants::RIGHT_PAD;
use crate::editor::Editor;
use crate::editor::ObjNames;
use crate::editor::{SEL_OVERLAY_COUNT, sel_overlay_name};
use crate::editor::viewer::{FileMode, file_lang, mount_image_obj};
use crate::editor::highlight::build_text_slice;
use crate::editor::highlight::build_gutter_slice;

pub fn setup(cv: &mut Canvas, ed: &Editor) {
    let names      = ObjNames::from_prefix(&ed.id_prefix);
    let cfg_snap   = *ed.cfg.get();
    let theme_snap = ed.theme.get().clone();
    let lh  = cfg_snap.line_height();
    let ex  = { *ed.live_x.get() };
    let ey  = { *ed.live_y.get() };
    let ew  = { *ed.live_w.get() };
    let eh  = { *ed.live_h.get() };

    let (init_mode, init_path) = {
        let st = ed.state.lock().unwrap();
        (st.mode.clone(), st.path.clone())
    };

    let code_x = ex + cfg_snap.text_x;
    let code_y = ey + cfg_snap.text_y;
    let code_w = ew - cfg_snap.text_x - RIGHT_PAD;

    cv.add_game_object(names.bg.clone(), GameObject::build(&names.bg)
        .position(ex, ey).size(ew, eh).layer(0)
        .image(tint_overlay(4000.0, 4000.0, theme_snap.editor_bg))
        .finish());
    mount_image_obj(cv, &ed.id_prefix, &init_mode, &init_path, ex, ey, ew, eh, 1);

    let viewport_lines = if eh > cfg_snap.text_y {
        (((eh - cfg_snap.text_y) / lh).ceil() as usize) + 2
    } else { 2 };

    let (init_text, init_gutter) = {
        let st  = ed.state.lock().unwrap();
        let end = viewport_lines.min(st.lines.len().max(1));
        let lines = if st.lines.is_empty() { vec![String::new()] } else { st.lines[0..end].to_vec() };
        (
            build_text_slice(&lines, &ed.code_font, &cfg_snap, &theme_snap, &file_lang(&init_path)),
            build_gutter_slice(0, end.max(1), 0, &ed.gutter_font, &cfg_snap, &theme_snap),
        )
    };

    if init_mode == FileMode::Text {
        let st = ed.state.lock().unwrap();
        if let Some(line) = st.lines.iter().max_by_key(|l| l.chars().count()) {
            let t = build_text_slice(std::slice::from_ref(line), &ed.code_font, &cfg_snap, &theme_snap, &file_lang(&init_path));
            *ed.max_line_width.get_mut() = t.size().0;
        }
    }

    cv.add_game_object(names.gutter_bg.clone(), {
        let mut o = GameObject::build(&names.gutter_bg)
            .position(ex, ey).size(cfg_snap.gutter_w, eh).layer(2)
            .image(tint_overlay(4000.0, 4000.0, theme_snap.gutter_bg))
            .finish();
        o.visible = init_mode == FileMode::Text;
        o
    });

    {
        let mut o = GameObject::build(&names.gutter)
            .position(ex, code_y).size(4000.0, 4000.0).layer(3)
            .clip()
            .clip_origin(ex, ey)
            .clip_size(cfg_snap.gutter_w, eh)
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
            .image(tint_overlay(1.0, lh.max(1.0), quartz::Color(38, 79, 120, 180)))
            .clip()
            .clip_origin(code_x, ey)
            .clip_size(code_w, eh)
            .finish();
        o.visible = false; 
        cv.add_game_object(name, o);
    }

    {
        let mut o = GameObject::build(&names.code_text)
            .position(code_x, code_y).size(4000.0, 4000.0).layer(1)
            .clip()
            .clip_origin(ex + cfg_snap.text_x, ey)
            .clip_size(code_w, eh)
            .finish();
        o.set_drawable(Box::new(init_text));
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(names.code_text.clone(), o);
    }

    {
        let (cur_w, cur_h, _) = cfg_snap.cursor_size();
        let mut o = GameObject::build(&names.cursor)
            .position(code_x, code_y).size(cur_w, cur_h).layer(4)
            .image(prism::canvas::Image {
                shape: prism::canvas::ShapeType::Rectangle(0.0, (cur_w, cur_h), 0.0),
                image: cfg_snap.cursor_style.build_image(cur_w, cur_h).into(),
                color: None,
            })
            .finish();
        o.visible = init_mode == FileMode::Text;
        cv.add_game_object(names.cursor.clone(), o);
    }
}