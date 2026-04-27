use crate::constants::{CTRL, KW};
use crate::editor::viewer::Lang;
use crate::preferences::Settings;
use quartz::{Align, Color, Font, Span, Text};
use std::sync::Arc;
use tree_sitter::{InputEdit, Language, Parser, Point};

// ── ThemeColors ───────────────────────────────────────────────────────────────

//keep ThemeColors
#[derive(Clone)]
pub struct ThemeColors {
    pub default: Color,
    pub keyword: Color,
    pub control: Color,
    pub ty: Color,
    pub string: Color,
    pub number: Color,
    pub comment: Color,
    pub macro_col: Color,
    pub lifetime: Color,
    pub lineno: Color,
    pub lineno_a: Color,
    pub hud: Color,
    pub gutter_bg: Color,
    pub editor_bg: Color,
}

//keep
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

//keep
fn extract_string_after_key<'a>(text: &'a str, name: &str) -> Option<&'a str> {
    let needle = format!("<key>{name}</key>");
    let pos = text.find(needle.as_str())?;
    let after = &text[pos + needle.len()..];
    let start = after.find("<string>")? + "<string>".len();
    let end = after[start..].find("</string>")?;
    Some(after[start..start + end].trim())
}

//keep
pub fn load_tm_theme(bytes: &[u8]) -> ThemeColors {
    let zero = Color(0, 0, 0, 255);
    let mut t = ThemeColors {
        default: zero,
        keyword: zero,
        control: zero,
        ty: zero,
        string: zero,
        number: zero,
        comment: zero,
        macro_col: zero,
        lifetime: zero,
        lineno: Color(80, 80, 80, 255),
        lineno_a: Color(180, 180, 180, 255),
        hud: Color(180, 180, 100, 255),
        gutter_bg: zero,
        editor_bg: zero,
    };
    let text = match std::str::from_utf8(bytes) {
        Ok(t) => t,
        Err(_) => return t,
    };
    let mut remaining = text;

    while let Some(dict_start) = remaining.find("<dict>") {
        let block_from = &remaining[dict_start..];
        let dict_end = match block_from.find("</dict>") {
            Some(e) => e + "</dict>".len(),
            None => break,
        };
        let block = &block_from[..dict_end];
        remaining = &block_from[dict_end..];
        let scope = extract_string_after_key(block, "scope");
        let fg = extract_string_after_key(block, "foreground");
        let bg = extract_string_after_key(block, "background");

        if scope.is_none() {
            if let Some(c) = bg.and_then(parse_hex_color) {
                t.editor_bg = c;
            }
            if let Some(c) = fg.and_then(parse_hex_color) {
                t.default = c;
            }
            if let Some(c) = extract_string_after_key(block, "gutter").and_then(parse_hex_color) {
                t.gutter_bg = c;
            } else {
                t.gutter_bg = Color(
                    t.editor_bg.0.saturating_sub(15),
                    t.editor_bg.1.saturating_sub(15),
                    t.editor_bg.2.saturating_sub(15),
                    255,
                );
            }
            if let Some(c) =
                extract_string_after_key(block, "gutterForeground").and_then(parse_hex_color)
            {
                t.lineno = c;
                t.lineno_a = Color(
                    (c.0 as u16 + 80).min(255) as u8,
                    (c.1 as u16 + 80).min(255) as u8,
                    (c.2 as u16 + 80).min(255) as u8,
                    255,
                );
            }
            continue;
        }

        let color = match fg.and_then(parse_hex_color) {
            Some(c) => c,
            None => continue,
        };
        for part in scope.unwrap().split(',') {
            let part = part.trim();
            if part.starts_with("comment") {
                t.comment = Color(color.0, color.1, color.2, 140);
            } else if part.starts_with("string") {
                t.string = color;
            } else if part.starts_with("constant.numeric") || part == "constant.other.color" {
                t.number = color;
            } else if part.starts_with("keyword.control") {
                t.control = color;
            } else if part.starts_with("keyword") || part.starts_with("storage") {
                t.keyword = color;
            } else if part.starts_with("entity.name.type")
                || part.starts_with("support.type")
                || part.starts_with("support.class")
                || part.starts_with("entity.name.class")
                || part.starts_with("variable.language")
            {
                t.ty = color;
            } else if part.starts_with("entity.name.function")
                || part.starts_with("support.function")
            {
                t.macro_col = color;
            } else if part.starts_with("storage.modifier.lifetime") {
                t.lifetime = color;
            }
        }
    }
    t
}

// ── Syntax helpers ────────────────────────────────────────────────────────────

//TODO: replace.
pub fn token_color(word: &str, next_char: Option<char>, theme: &ThemeColors) -> Color {
    if next_char == Some('!') {
        return theme.macro_col;
    }
    if word == "self" || word == "Self" {
        return theme.ty;
    }
    if CTRL.contains(&word) {
        return theme.control;
    }
    if KW.contains(&word) {
        return theme.keyword;
    }
    if word.starts_with(|c: char| c.is_uppercase()) {
        return theme.ty;
    }
    theme.default
}

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

//TODO: refactor this function.
pub fn build_text_slice(
    lines: &[String],
    font: &Arc<Font>,
    cfg: &Settings,
    theme: &ThemeColors,
    lang: &Lang,
) -> Text {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");
    /* Turns all the text of a Rust file into one giant String, which is what tree-sitter's parse()
    expects. */
    let source_code = lines.join("\n");
    let mut tree = parser.parse(source_code, None).unwrap();
    let mut tree_cursor = tree.walk();
    let mut spans: Vec<Span> = Vec::new();

    /* The goal of this loop is to traverse every node in the Tree, determine if it is a leaf,
    and if so, copy it's data into `spans`. */
    'outer: loop {
        if tree_cursor.goto_first_child() {
        } else {
            // process leaf here. Find current node first.
            let current_node = tree_cursor.node();
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
    theme: &ThemeColors,
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
                theme.lineno,
                0.0,
            ));
        }
        let color = if abs_line == cursor_row {
            theme.lineno_a
        } else {
            theme.lineno
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

