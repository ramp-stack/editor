pub mod constants;
pub mod preferences;
pub mod editor;
pub mod objects;
pub mod logic;

pub use editor::Editor;
pub use editor::highlight::ThemeColors;

pub mod prelude {
    pub use crate::editor::Editor;
    pub use crate::preferences::Settings;
    pub use crate::preferences::CursorStyle;
}

impl Editor {
    pub fn mount(&self, cv: &mut quartz::Canvas) {
        objects::editor_obj::setup(cv, self);
    }

    pub fn register_callbacks(&self, cv: &mut quartz::Canvas) {
        logic::input::register(cv, self);
        logic::editor_obj::register(cv, self);
    }
}