use quartz::Color;
use serde_json::from_slice;
use std::collections::HashMap;

pub fn parse_hex_color(s: &str) -> Option<Color> {
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
    let value: serde_json::Value =
        from_slice(bytes).expect("Failed to read JSON theme file.");
    let mut map = HashMap::new();

    for section in &["syntax", "ui"] {
        if let Some(obj) = value[section].as_object() {
            for (key, val) in obj {
                map.insert(
                    key.to_string(),
                    parse_hex_color(
                        val.as_str().expect("Theme value is not a string."),
                    )
                    .expect("Failed to parse_hex_color"),
                );
            }
        }
    }
    map
}
