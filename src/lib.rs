pub mod components;
pub mod constants;
pub mod editor;
pub mod logic;
pub mod objects;
pub mod preferences;

pub use editor::Editor;

pub mod prelude {
    pub use crate::editor::Editor;
    pub use crate::preferences::{CursorStyle, Settings};
}
