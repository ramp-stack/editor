use flowmango::{Canvas, GameObject};
use quartz::Shared;

use crate::components::language::FileMode;

pub fn obj_name(id_prefix: &str) -> String {
    format!("{}_img", id_prefix)
}

pub fn mount(
    cv:         &mut Canvas,
    id_prefix:  &str,
    mode:       &FileMode,
    path:       &str,
    x: f32, y: f32, w: f32, h: f32,
    layer: i32,
) {
    let name = obj_name(id_prefix);
    let obj  = if *mode == FileMode::Image {
        let bytes = std::fs::read(path).expect("Failed to read image.");
        let img   = quartz::load_image_sized(&bytes, w, h);
        GameObject::build(&name)
            .position(x, y).size(w, h).layer(layer).image(img).finish()
    } else {
        let mut o = GameObject::build(&name)
            .position(x, y).size(w, h).layer(layer)
            .image(quartz::tint_overlay(1.0, 1.0, quartz::Color(0, 0, 0, 0)))
            .finish();
        o.visible = false;
        o
    };
    cv.add_game_object(name, obj);
}

pub fn update(
    cv:         &mut Canvas,
    id_prefix:  &str,
    path:       &str,
    x: f32, y: f32, w: f32, h: f32,
    loaded_key: &Shared<String>,
) {
    let key = format!("{}|{}|{}", path, w as u32, h as u32);
    if *loaded_key.get() == key {
        if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix)) {
            o.position = (x, y);
            o.visible  = true;
        }
        return;
    }
    let bytes = std::fs::read(path).expect("Failed to read image.");
    let img   = quartz::load_image_sized(&bytes, w, h);
    *loaded_key.get_mut() = key;
    if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix)) {
        o.position = (x, y);
        o.size     = (w, h);
        o.set_image(img);
        o.visible  = true;
    }
}

pub fn show(cv: &mut Canvas, id_prefix: &str, text_names: &[&str]) {
    for name in text_names {
        if let Some(o) = cv.get_game_object_mut(name) { o.visible = false; }
    }
    if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix)) {
        o.visible = true;
    }
}

pub fn hide(cv: &mut Canvas, id_prefix: &str, text_names: &[&str]) {
    if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix)) {
        o.visible = false;
    }
    for name in text_names {
        if let Some(o) = cv.get_game_object_mut(name) { o.visible = true; }
    }
}
