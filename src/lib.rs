pub mod constants;
pub mod editor;
pub mod logic;
pub mod objects;
pub mod preferences;

pub use editor::Editor;

pub mod prelude {
    pub use crate::editor::Editor;
    pub use crate::preferences::CursorStyle;
    pub use crate::preferences::Settings;
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

