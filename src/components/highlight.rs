use crate::components::language::Lang;
use crate::preferences::Settings;
use quartz::{Align, Color, Font, Span, Text};
use serde_json::from_slice;
use std::collections::HashMap;
use std::sync::Arc;
use tree_sitter::{InputEdit, Language, Parser, Point};

const FALLBACK: Color = Color(200, 200, 200, 255);

// A hex color string like "#ff9d00" gets passed in here.
fn parse_hex_color(s: &str) -> Option<Color> {
    let s = s.trim().trim_start_matches('#');
    match s.len() {
        8 => Some(Color(
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
            u8::from_str_radix(&s[6..8], 16).ok()?,
        )),
        6 => Some(Color(
            u8::from_str_radix(&s[0..2], 16).ok()?,
            u8::from_str_radix(&s[2..4], 16).ok()?,
            u8::from_str_radix(&s[4..6], 16).ok()?,
            255,
        )),
        _ => None,
    }
}

pub fn load_json_theme(bytes: &[u8]) -> HashMap<String, Color> {
    let value: serde_json::Value = from_slice(bytes).expect("Failed to read JSON theme file.");
    let mut hashmap_from_json = HashMap::new();
    //Load the syntax object from JSON.
    if let Some(map) = value["syntax"].as_object() {
        for (key, val) in map {
            hashmap_from_json.insert(
                key.to_string(),
                parse_hex_color(val.as_str().expect("Theme value is not a string."))
                    .expect("Failed to parse_hex_color"),
            );
        }
    }
    //Load the UI object from JSON.
    if let Some(map) = value["ui"].as_object() {
        for (key, val) in map {
            hashmap_from_json.insert(
                key.to_string(),
                parse_hex_color(val.as_str().expect("Theme value is not a string."))
                    .expect("Failed to parse_hex_color"),
            );
        }
    }
    hashmap_from_json
}

// ── Syntax helpers ────────────────────────────────────────────────────────────

pub fn char_range(s: &str, ca: usize, cb: usize) -> (usize, usize) {
    let mut iter = s.char_indices();
    let start = iter.nth(ca).map(|(b, _)| b).unwrap_or(s.len());
    let end = if cb > ca {
        iter.nth(cb - ca - 1).map(|(b, _)| b).unwrap_or(s.len())
    } else {
        start
    };
    (start, end)
}
/* This function takes in all lines of the source code, turns them into one giant string,
* creates a Tree-Sitter tree, travels through every leaf node, assigns each relevant leaf
* (or parent of leaf) the correct color, then packs all spans (including whitespace) into
* one giant Text and returns it. */
pub fn build_colored_text(
    lines: &[String],
    font: &Arc<Font>,
    cfg: &Settings,
    theme: &HashMap<String, Color>,
    lang: &Lang,
) -> Text {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");
    // Converts source code into one giant String, which is what tree-sitter's parse() expects.
    let source_code = lines.join("\n");
    let tree = parser.parse(&source_code, None).unwrap();
    let mut tree_cursor = tree.walk();
    let mut spans: Vec<Span> = Vec::new();
    let mut prev_end: usize = 0;

    /* This loop traverses every node in the Tree, determine if it is a leaf,
    and if so, copy it's data into `spans`, OR, jumps to it's parent and copy it's data into spans. */
    'outer: loop {
        if tree_cursor.goto_first_child() {
        } else {
            // Bingo, we're at a leaf. process leaf here. Find current node first.
            let current_node = tree_cursor.node();
            // find the beg and end of the node in the byte range.
            let start_byte = current_node.start_byte();
            let end_byte = current_node.end_byte();

            //FIX: String inside attribute_item should be string colored. This is happening
            //because the entire byte length of attribute_item is getting colored yellow.
            if matches!(current_node.kind(), "#") {
                /* First span captures whitespace. Notice that we jump up to parent to handle special
                attribute situation. */
                spans.push(Span::new(
                    source_code[prev_end..current_node.parent().unwrap().start_byte()].to_string(),
                    cfg.font_size,
                    Some(cfg.line_height()),
                    font.clone(),
                    theme.get("default").copied().unwrap(),
                    0.0,
                ));

                //Second span captures byte space of parent (token text) and colors it correctly.
                spans.push(Span::new(
                    source_code[current_node.parent().unwrap().start_byte()
                        ..current_node.parent().unwrap().end_byte()]
                        .to_string(),
                    cfg.font_size,
                    Some(cfg.line_height()),
                    font.clone(),
                    theme.get("attribute_item").copied().unwrap_or(FALLBACK),
                    0.0,
                ));

                prev_end = current_node.parent().unwrap().end_byte();
                tree_cursor.goto_parent();
            } else if matches!(current_node.kind(), "//" | "/*") {
                /* First span captures whitespace. Notice that we jump up to parent to handle special
                comment situation. */
                spans.push(Span::new(
                    source_code[prev_end..current_node.parent().unwrap().start_byte()].to_string(),
                    cfg.font_size,
                    Some(cfg.line_height()),
                    font.clone(),
                    theme.get("default").copied().unwrap(),
                    0.0,
                ));
                //Second span captures byte space of parent (token text) and colors it correctly.
                spans.push(Span::new(
                    source_code[current_node.parent().unwrap().start_byte()
                        ..current_node.parent().unwrap().end_byte()]
                        .to_string(),
                    cfg.font_size,
                    Some(cfg.line_height()),
                    font.clone(),
                    //line_comment is the same color as block_comment. Not very principled,
                    //but the JSON doesn't allow for something more elegant yet.
                    theme.get("line_comment").copied().unwrap(),
                    0.0,
                ));

                prev_end = current_node.parent().unwrap().end_byte();
                // We're setting tree_cursor to parent of "/*" (which is block_comment) to skip "*/".
                if current_node.kind() == "/*" {
                    tree_cursor.goto_parent();
                }
            } else {
                let token_text = &source_code[start_byte..end_byte];
                spans.push(Span::new(
                    source_code[prev_end..start_byte].to_string(),
                    cfg.font_size,
                    Some(cfg.line_height()),
                    font.clone(),
                    theme.get("default").copied().unwrap(),
                    0.0,
                ));
                let theme_key = if current_node.parent().unwrap().kind() == "function_item"
                    && current_node.kind() == "identifier"
                    && tree_cursor.field_name() == Some("name")
                {
                    "function_name"
                } else {
                    current_node.kind()
                };
                spans.push(Span::new(
                    token_text.to_string(),
                    cfg.font_size,
                    Some(cfg.line_height()),
                    font.clone(),
                    theme
                        .get(theme_key)
                        .copied()
                        //default acts a fallback.
                        .unwrap_or(theme.get("default").copied().unwrap()),
                    0.0,
                ));
                prev_end = end_byte;
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
    }

    Text::new(spans, None, Align::Left, None)
}

pub fn build_gutter_slice(
    abs_start: usize,
    abs_end: usize,
    cursor_row: usize,
    font: &Arc<Font>,
    cfg: &Settings,
    theme: &HashMap<String, Color>,
) -> Text {
    let fs = cfg.font_size;
    let lh = cfg.line_height();
    let mut spans: Vec<Span> = Vec::with_capacity((abs_end - abs_start) * 2);
    for (i, abs_line) in (abs_start..abs_end).enumerate() {
        if i > 0 {
            spans.push(Span::new(
                "\n".into(),
                fs,
                Some(lh),
                font.clone(),
                theme.get("lineno").copied().unwrap(),
                0.0,
            ));
        }
        let color = if abs_line == cursor_row {
            theme.get("lineno_a").copied().unwrap()
        } else {
            theme.get("lineno").copied().unwrap()
        };
        spans.push(Span::new(
            format!("{:>4}", abs_line + 1),
            fs,
            Some(lh),
            font.clone(),
            color,
            0.0,
        ));
    }
    Text::new(spans, None, Align::Right, None)
}