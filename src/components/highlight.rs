use quartz::{Align, Color, Font, Span, Text};
use std::collections::HashMap;
use std::sync::Arc;
use tree_sitter::Parser;

use crate::components::language::Lang;
use crate::preferences::Settings;

pub fn char_range(s: &str, ca: usize, cb: usize) -> (usize, usize) {
    let mut iter  = s.char_indices();
    let start     = iter.nth(ca).map(|(b, _)| b).unwrap_or(s.len());
    let end       = if cb > ca {
        iter.nth(cb - ca - 1).map(|(b, _)| b).unwrap_or(s.len())
    } else {
        start
    };
    (start, end)
}

pub fn build_text_slice(
    lines: &[String],
    font:  &Arc<Font>,
    cfg:   &Settings,
    theme: &HashMap<String, Color>,
    lang:  &Lang,
) -> Text {
    match lang {
        Lang::Rust  => build_rust_text(lines, font, cfg, theme),
        Lang::Plain => build_plain_text(lines, font, cfg, theme),
    }
}

pub fn build_gutter_slice(
    abs_start:  usize,
    abs_end:    usize,
    cursor_row: usize,
    font:       &Arc<Font>,
    cfg:        &Settings,
    theme:      &HashMap<String, Color>,
) -> Text {
    let fs = cfg.font_size;
    let lh = cfg.line_height();
    let mut spans: Vec<Span> = Vec::with_capacity((abs_end - abs_start) * 2);

    for (i, abs_line) in (abs_start..abs_end).enumerate() {
        if i > 0 {
            spans.push(Span::new(
                "\n".into(), fs, Some(lh), font.clone(),
                theme.get("lineno").copied().unwrap_or_default(), 0.0,
            ));
        }
        let color = if abs_line == cursor_row {
            theme.get("lineno_a").copied().unwrap_or_default()
        } else {
            theme.get("lineno").copied().unwrap_or_default()
        };
        spans.push(Span::new(
            format!("{:>4}", abs_line + 1),
            fs, Some(lh), font.clone(), color, 0.0,
        ));
    }
    Text::new(spans, None, Align::Right, None)
}

fn build_plain_text(
    lines: &[String],
    font:  &Arc<Font>,
    cfg:   &Settings,
    theme: &HashMap<String, Color>,
) -> Text {
    let color = theme.get("default").copied().unwrap_or_default();
    let text  = lines.join("\n");
    Text::new(
        vec![Span::new(text, cfg.font_size, Some(cfg.line_height()), font.clone(), color, 0.0)],
        None, Align::Left, None,
    )
}

fn build_rust_text(
    lines: &[String],
    font:  &Arc<Font>,
    cfg:   &Settings,
    theme: &HashMap<String, Color>,
) -> Text {
    let mut parser = Parser::new();
    parser
        .set_language(&tree_sitter_rust::LANGUAGE.into())
        .expect("Error loading Rust grammar");

    let source_code = lines.join("\n");
    let tree        = parser.parse(&source_code, None).unwrap();
    let mut cursor  = tree.walk();
    let mut spans:    Vec<Span> = Vec::new();
    let mut prev_end: usize     = 0;

    let default_color  = theme.get("default").copied().unwrap_or_default();
    let comment_color  = theme.get("line_comment").copied().unwrap_or(default_color);

    'outer: loop {
        if cursor.goto_first_child() {
            continue;
        }

        let node       = cursor.node();
        let start_byte = node.start_byte();
        let end_byte   = node.end_byte();

        if matches!(node.kind(), "//" | "/*") {
            let parent_start = node.parent().unwrap().start_byte();
            spans.push(span(&source_code[prev_end..parent_start], cfg, font, default_color));
            let parent_end = node.parent().unwrap().end_byte();
            spans.push(span(&source_code[parent_start..parent_end], cfg, font, comment_color));
            prev_end = parent_end;
            if node.kind() == "/*" {
                cursor.goto_parent();
            }
        } else {
            let color = theme.get(node.kind()).copied().unwrap_or(default_color);
            spans.push(span(&source_code[prev_end..start_byte], cfg, font, default_color));
            spans.push(span(&source_code[start_byte..end_byte], cfg, font, color));
            prev_end = end_byte;
        }

        loop {
            if cursor.goto_next_sibling() { break; }
            if !cursor.goto_parent()      { break 'outer; }
        }
    }

    Text::new(spans, None, Align::Left, None)
}

#[inline]
fn span(text: &str, cfg: &Settings, font: &Arc<Font>, color: Color) -> Span {
    Span::new(text.to_string(), cfg.font_size, Some(cfg.line_height()), font.clone(), color, 0.0)
}
