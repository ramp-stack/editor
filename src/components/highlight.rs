use crate::components::language::Lang;
use crate::preferences::Settings;
use quartz::{Align, Color, Font, Span, Text};
use serde_json::from_slice;
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use tree_sitter::{Parser, Tree};

const FALLBACK: Color = Color(200, 200, 200, 255);

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
    let mut map = HashMap::new();
    if let Some(obj) = value["syntax"].as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), parse_hex_color(v.as_str().unwrap()).unwrap());
        }
    }
    if let Some(obj) = value["ui"].as_object() {
        for (k, v) in obj {
            map.insert(k.clone(), parse_hex_color(v.as_str().unwrap()).unwrap());
        }
    }
    map
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

#[inline]
fn safe_slice(s: &str, start: usize, end: usize) -> &str {
    let len = s.len();
    let start = start.min(len);
    let end = end.min(len).max(start);
    let start = (0..=start)
        .rev()
        .find(|&i| s.is_char_boundary(i))
        .unwrap_or(0);
    let end = (end..=len).find(|&i| s.is_char_boundary(i)).unwrap_or(len);
    &s[start..end]
}

pub struct ParseCache {
    pub source: String,
    pub tree: Tree,
    pub line_offsets: Vec<usize>,
}

impl ParseCache {
    pub fn build(source: String) -> Option<Self> {
        let mut parser = Parser::new();
        parser
            .set_language(&tree_sitter_rust::LANGUAGE.into())
            .ok()?;
        let tree = parser.parse(&source, None)?;
        let line_offsets = build_line_offsets(&source);
        Some(Self {
            source,
            tree,
            line_offsets,
        })
    }

    #[inline]
    pub fn line_start_byte(&self, line: usize) -> usize {
        self.line_offsets
            .get(line)
            .copied()
            .unwrap_or(self.source.len())
    }
}

fn build_line_offsets(source: &str) -> Vec<usize> {
    let mut offsets = vec![0usize];
    for (i, b) in source.bytes().enumerate() {
        if b == b'\n' {
            offsets.push(i + 1);
        }
    }
    offsets
}

const NO_VERSION: u64 = u64::MAX;

pub struct AsyncParser {
    ready_ver: Arc<AtomicU64>,
    result: Arc<Mutex<Option<ParseCache>>>,
    pending: Arc<Mutex<Option<(u64, String)>>>,
    condvar: Arc<std::sync::Condvar>,
}

impl AsyncParser {
    pub fn new() -> Self {
        let ready_ver = Arc::new(AtomicU64::new(NO_VERSION));
        let result: Arc<Mutex<Option<ParseCache>>> = Arc::new(Mutex::new(None));
        let pending: Arc<Mutex<Option<(u64, String)>>> = Arc::new(Mutex::new(None));
        let condvar = Arc::new(std::sync::Condvar::new());

        {
            let ready_ver = ready_ver.clone();
            let result = result.clone();
            let pending = pending.clone();
            let condvar = condvar.clone();

            std::thread::spawn(move || {
                let mut parser = Parser::new();
                let _ = parser.set_language(&tree_sitter_rust::LANGUAGE.into());

                loop {
                    let (ver, src) = {
                        let mut lock = pending.lock().unwrap();
                        loop {
                            if let Some(work) = lock.take() {
                                break work;
                            }
                            lock = condvar.wait(lock).unwrap();
                        }
                    };

                    let Some(tree) = parser.parse(&src, None) else {
                        continue;
                    };

                    let line_offsets = build_line_offsets(&src);
                    let cache = ParseCache {
                        source: src,
                        tree,
                        line_offsets,
                    };

                    *result.lock().unwrap() = Some(cache);
                    ready_ver.store(ver, Ordering::Release);
                }
            });
        }

        Self {
            ready_ver,
            result,
            pending,
            condvar,
        }
    }

    pub fn request(&self, version: u64, source: String) {
        *self.pending.lock().unwrap() = Some((version, source));
        self.condvar.notify_one();
    }

    pub fn ready_version(&self) -> u64 {
        self.ready_ver.load(Ordering::Acquire)
    }

    pub fn borrow(&self) -> std::sync::MutexGuard<Option<ParseCache>> {
        self.result.lock().unwrap()
    }
}

pub fn build_colored_text(
    lines: &[String],
    font: &Arc<Font>,
    cfg: &Settings,
    theme: &HashMap<String, Color>,
    _lang: &Lang,
    parse_cache: &[Color],
    slice_start_byte: usize,
) -> Text {
    if parse_cache.is_empty() {
        build_plain_text(lines, font, cfg, theme)
    } else {
        let mut spans: Vec<Span> = Vec::new();
        let mut global_byte = slice_start_byte;
        let last_line = lines.len().saturating_sub(1);
        for (li, line) in lines.iter().enumerate() {
            let mut run_start = 0usize;
            let mut run_color = parse_cache[global_byte.min(parse_cache.len() - 1)];
            let mut line_byte = 0usize;

            for ch in line.chars() {
                let c = parse_cache[global_byte.min(parse_cache.len() - 1)];
                if c != run_color {
                    spans.push(Span::new(
                        line[run_start..line_byte].to_string(),
                        cfg.font_size,
                        Some(cfg.line_height()),
                        font.clone(),
                        run_color,
                        0.0,
                    ));
                    run_start = line_byte;
                    run_color = c;
                }
                line_byte += ch.len_utf8();
                global_byte += ch.len_utf8();
            }

            let tail = if li < last_line {
                global_byte += 1; // newline byte
                format!("{}\n", &line[run_start..])
            } else {
                line[run_start..].to_string()
            };
            spans.push(Span::new(
                tail,
                cfg.font_size,
                Some(cfg.line_height()),
                font.clone(),
                run_color,
                0.0,
            ));
        }
        Text::new(spans, None, Align::Left, None)
    }
}

fn build_plain_text(
    lines: &[String],
    font: &Arc<Font>,
    cfg: &Settings,
    theme: &HashMap<String, Color>,
) -> Text {
    let default_col = theme.get("default").copied().unwrap_or(FALLBACK);
    let source = lines.join("\n");
    Text::new(
        vec![Span::new(
            source,
            cfg.font_size,
            Some(cfg.line_height()),
            font.clone(),
            default_col,
            0.0,
        )],
        None,
        Align::Left,
        None,
    )
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
