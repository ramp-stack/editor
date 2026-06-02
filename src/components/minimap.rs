use quartz::{Align, Arc, Color, Font, Span, Text};
use std::collections::HashMap;

use crate::components::highlight::ParseCache;

pub const MINIMAP_W: f32 = 110.0;
pub const MINIMAP_FS: f32 = 2.5;
pub const MINIMAP_LH: f32 = 4.0;
pub const MINIMAP_PAD_X: f32 = 4.0;

fn get(theme: &HashMap<String, Color>, key: &str) -> Color {
    theme.get(key).copied().unwrap_or(Color(192, 202, 245, 255))
}

fn build_color_map(cache: &ParseCache, theme: &HashMap<String, Color>) -> Vec<Color> {
    let def_c = get(theme, "default");
    let src_len = cache.source.len();
    let mut map: Vec<Color> = vec![def_c; src_len + 1];

    let mut tree_cursor = cache.tree.walk();

    'outer: loop {
        if tree_cursor.goto_first_child() {
            continue;
        }

        let node = tree_cursor.node();
        let sb = node.start_byte();
        let eb = node.end_byte().min(src_len);

        if matches!(node.kind(), "#") {
            if let Some(parent) = node.parent() {
                let c = get(theme, "attribute_item");
                for i in parent.start_byte()..parent.end_byte().min(src_len) {
                    map[i] = c;
                }
                tree_cursor.goto_parent();
            }
        } else if matches!(node.kind(), "//" | "/*") {
            if let Some(parent) = node.parent() {
                let c = get(theme, "line_comment");
                for i in parent.start_byte()..parent.end_byte().min(src_len) {
                    map[i] = c;
                }
                if node.kind() == "/*" {
                    tree_cursor.goto_parent();
                }
            }
        } else {
            let theme_key = if node.parent().map(|p| p.kind()) == Some("function_item")
                && node.kind() == "identifier"
                && tree_cursor.field_name() == Some("name")
            {
                "function_name"
            } else {
                node.kind()
            };
            let c = theme.get(theme_key).copied().unwrap_or(def_c);
            for i in sb..eb {
                map[i] = c;
            }
        }

        loop {
            if tree_cursor.goto_next_sibling() {
                break;
            }
            if !tree_cursor.goto_parent() {
                break 'outer;
            }
        }
    }

    map
}

fn line_to_text(line: &str, line_byte_start: usize, color_map: &[Color], font: &Arc<Font>) -> Text {
    if line.is_empty() {
        return Text::new(
            vec![Span::new(
                " ".into(),
                MINIMAP_FS,
                Some(MINIMAP_LH),
                font.clone(),
                Color(0, 0, 0, 0),
                0.0,
            )],
            None,
            Align::Left,
            None,
        );
    }

    let mut spans: Vec<Span> = Vec::new();
    let mut run_start = 0usize;
    let mut run_color = color_map[line_byte_start.min(color_map.len() - 1)];

    let mut byte_off = 0usize;
    for ch in line.chars() {
        let ch_len = ch.len_utf8();
        let global_off = line_byte_start + byte_off;
        let c = color_map[global_off.min(color_map.len() - 1)];

        if c.0 != run_color.0 || c.1 != run_color.1 || c.2 != run_color.2 {
            let text = &line[run_start..byte_off];
            if !text.is_empty() {
                spans.push(Span::new(
                    text.to_string(),
                    MINIMAP_FS,
                    Some(MINIMAP_LH),
                    font.clone(),
                    run_color,
                    0.0,
                ));
            }
            run_start = byte_off;
            run_color = c;
        }

        byte_off += ch_len;
    }

    let tail = &line[run_start..];
    if !tail.is_empty() {
        spans.push(Span::new(
            tail.to_string(),
            MINIMAP_FS,
            Some(MINIMAP_LH),
            font.clone(),
            run_color,
            0.0,
        ));
    }

    Text::new(spans, None, Align::Left, None)
}

pub fn build_minimap_texts(
    lines: &[String],
    cache: &ParseCache,
    theme: &HashMap<String, Color>,
    font: &Arc<Font>,
) -> Vec<Text> {
    let color_map = build_color_map(cache, theme);

    let mut texts = Vec::with_capacity(lines.len());
    let mut byte_off = 0usize;

    for line in lines {
        texts.push(line_to_text(line, byte_off, &color_map, font));
        byte_off += line.len() + 1;
    }

    texts
}
