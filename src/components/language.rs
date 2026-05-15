#[derive(Clone, PartialEq, Debug)]
pub enum FileMode {
    Text,
    Image,
}

#[derive(Clone, PartialEq, Debug)]
pub enum Lang {
    Rust,
    Plain,
}

pub fn file_mode(path: &str) -> FileMode {
    match path_ext(path).as_str() {
        "png" | "jpg" | "jpeg" | "gif" | "bmp" | "webp" | "tiff" | "tif" | "ico" => {
            FileMode::Image
        }
        _ => FileMode::Text,
    }
}

pub fn file_lang(path: &str) -> Lang {
    match path_ext(path).as_str() {
        "rs" => Lang::Rust,
        _    => Lang::Plain,
    }
}

fn path_ext(path: &str) -> String {
    std::path::Path::new(path)
        .extension()
        .and_then(|e| e.to_str())
        .unwrap_or("")
        .to_ascii_lowercase()
}
