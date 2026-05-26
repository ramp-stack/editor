use flowmango::{Canvas, GameObject};
use quartz::{Align, Arc, Font, Shared, Span, Text};
use image::{RgbaImage, Rgba};
use std::sync::Arc as StdArc;

use crate::components::language::FileMode;

const MAX_IMG_W:  f32      = 800.0;
const MAX_IMG_H:  f32      = 500.0;
const META_GAP:   f32      = 16.0;
const META_FS:    f32      = 12.0;
const META_LH:    f32      = 18.0;
const META_COLOR: quartz::Color = quartz::Color(100, 110, 140, 255);
const META_HL:    quartz::Color = quartz::Color(160, 170, 210, 255);
const CHECK_SIZE: u32      = 8;
const CHECK_A: Rgba<u8> = Rgba([40, 40, 40, 255]);
const CHECK_B: Rgba<u8> = Rgba([17,  19,  28,  255]);

pub fn obj_name(id_prefix: &str)   -> String { format!("{}_img",       id_prefix) }
pub fn meta_name(id_prefix: &str)  -> String { format!("{}_img_meta",  id_prefix) }
pub fn check_name(id_prefix: &str) -> String { format!("{}_img_check", id_prefix) }

fn build_checker(w: f32, h: f32) -> quartz::Image {
    let pw = w.ceil() as u32;
    let ph = h.ceil() as u32;
    let mut img = RgbaImage::new(pw.max(1), ph.max(1));
    for y in 0..ph {
        for x in 0..pw {
            let even = ((x / CHECK_SIZE) + (y / CHECK_SIZE)) % 2 == 0;
            img.put_pixel(x, y, if even { CHECK_A } else { CHECK_B });
        }
    }
    quartz::Image {
        shape: quartz::ShapeType::Rectangle(0.0, (w, h), 0.0),
        image: StdArc::new(img),
        color: None,
    }
}

fn fit_size(src_w: f32, src_h: f32, max_w: f32, max_h: f32) -> (f32, f32) {
    let scale = (max_w / src_w).min(max_h / src_h).min(1.0);
    (src_w * scale, src_h * scale)
}

fn center_pos(x: f32, y: f32, w: f32, h: f32, iw: f32, ih: f32) -> (f32, f32) {
    (x + (w - iw) * 0.5, y + (h - ih) * 0.5)
}

fn read_meta(path: &str) -> (u32, u32, u64) {
    let file_size = std::fs::metadata(path).map(|m| m.len()).unwrap_or(0);
    let (ow, oh)  = image::image_dimensions(path).unwrap_or((0, 0));
    (ow, oh, file_size)
}

fn build_meta_text(path: &str, font: &Arc<Font>) -> Text {
    let (orig_w, orig_h, file_size) = read_meta(path);

    let size_str = if file_size >= 1_048_576 {
        format!("{:.1} MB", file_size as f64 / 1_048_576.0)
    } else if file_size >= 1_024 {
        format!("{:.1} KB", file_size as f64 / 1_024.0)
    } else {
        format!("{} B", file_size)
    };

    let ext = std::path::Path::new(path)
        .extension().and_then(|e| e.to_str()).unwrap_or("unknown")
        .to_ascii_uppercase();

    let filename = std::path::Path::new(path)
        .file_name().and_then(|n| n.to_str()).unwrap_or(path)
        .to_string();

    let label = |s: &str| Span::new(s.to_string(), META_FS, Some(META_LH), font.clone(), META_COLOR, 0.0);
    let value = |s: &str| Span::new(s.to_string(), META_FS, Some(META_LH), font.clone(), META_HL,    0.0);
    let nl    = ||         Span::new("\n".to_string(), META_FS, Some(META_LH), font.clone(), META_COLOR, 0.0);

    Text::new(vec![
        value(&filename),                                                    nl(),
        label("Format     "),  value(&ext),                                 nl(),
        label("Dimensions "),  value(&format!("{orig_w} × {orig_h} px")),  nl(),
        label("File size  "),  value(&size_str),
    ], None, Align::Left, None)
}

pub fn mount(
    cv:        &mut Canvas,
    id_prefix: &str,
    mode:      &FileMode,
    path:      &str,
    x: f32, y: f32, w: f32, h: f32,
    layer:     i32,
    font:      &Arc<Font>,
) {
    let is_img   = *mode == FileMode::Image;
    let (iw, ih) = fit_size(w, h * 0.75, MAX_IMG_W, MAX_IMG_H);
    let (ix, iy) = center_pos(x, y, w, h * 0.75, iw, ih);

    {
        let cname = check_name(id_prefix);
        let mut o = GameObject::build(&cname)
            .position(ix, iy).size(iw, ih).layer(layer)
            .finish();
        o.set_image(build_checker(iw, ih));
        o.visible = is_img;
        cv.add_game_object(cname, o);
    }

    {
        let name = obj_name(id_prefix);
        let obj = if is_img {
            let bytes = std::fs::read(path).expect("Failed to read image.");
            let img   = quartz::load_image_sized(&bytes, iw, ih);
            GameObject::build(&name)
                .position(ix, iy).size(iw, ih).layer(layer + 1).image(img).finish()
        } else {
            let mut o = GameObject::build(&name)
                .position(x, y).size(w, h).layer(layer + 1)
                .image(quartz::tint_overlay(1.0, 1.0, quartz::Color(0, 0, 0, 0)))
                .finish();
            o.visible = false;
            o
        };
        cv.add_game_object(name, obj);
    }

    {
        let mname = meta_name(id_prefix);
        let mut o = GameObject::build(&mname)
            .position(ix, iy + ih + META_GAP)
            .size(iw, META_LH * 5.0).layer(layer + 1)
            .finish();
        if is_img { o.set_drawable(Box::new(build_meta_text(path, font))); }
        o.visible = is_img;
        cv.add_game_object(mname, o);
    }
}

pub fn update(
    cv:         &mut Canvas,
    id_prefix:  &str,
    path:       &str,
    x: f32, y: f32, w: f32, h: f32,
    loaded_key: &Shared<String>,
    font:       &Arc<Font>,
) {
    let (iw, ih) = fit_size(w, h * 0.75, MAX_IMG_W, MAX_IMG_H);
    let (ix, iy) = center_pos(x, y, w, h * 0.75, iw, ih);
    let key      = format!("{}|{}|{}", path, iw as u32, ih as u32);
    let meta_y   = iy + ih + META_GAP;

    if *loaded_key.get() == key {
        if let Some(o) = cv.get_game_object_mut(&check_name(id_prefix)) {
            o.position = (ix, iy); o.visible = true;
        }
        if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix)) {
            o.position = (ix, iy); o.visible = true;
        }
        if let Some(o) = cv.get_game_object_mut(&meta_name(id_prefix)) {
            o.position = (ix, meta_y); o.visible = true;
        }
        return;
    }

    let bytes = std::fs::read(path).expect("Failed to read image.");
    let img   = quartz::load_image_sized(&bytes, iw, ih);
    *loaded_key.get_mut() = key;

    if let Some(o) = cv.get_game_object_mut(&check_name(id_prefix)) {
        o.set_image(build_checker(iw, ih));
        o.position = (ix, iy); o.size = (iw, ih); o.visible = true;
    }
    if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix)) {
        o.set_image(img);
        o.position = (ix, iy); o.size = (iw, ih); o.visible = true;
    }
    if let Some(o) = cv.get_game_object_mut(&meta_name(id_prefix)) {
        o.set_drawable(Box::new(build_meta_text(path, font)));
        o.position = (ix, meta_y); o.visible = true;
    }
}

pub fn show(cv: &mut Canvas, id_prefix: &str, text_names: &[&str]) {
    for name in text_names {
        if let Some(o) = cv.get_game_object_mut(name) { o.visible = false; }
    }
    if let Some(o) = cv.get_game_object_mut(&check_name(id_prefix)) { o.visible = true; }
    if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix))   { o.visible = true; }
    if let Some(o) = cv.get_game_object_mut(&meta_name(id_prefix))  { o.visible = true; }
}

pub fn hide(cv: &mut Canvas, id_prefix: &str, text_names: &[&str]) {
    if let Some(o) = cv.get_game_object_mut(&check_name(id_prefix)) { o.visible = false; }
    if let Some(o) = cv.get_game_object_mut(&obj_name(id_prefix))   { o.visible = false; }
    if let Some(o) = cv.get_game_object_mut(&meta_name(id_prefix))  { o.visible = false; }
    for name in text_names {
        if let Some(o) = cv.get_game_object_mut(name) { o.visible = true; }
    }
}