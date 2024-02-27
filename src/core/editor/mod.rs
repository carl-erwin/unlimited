mod editor;
mod env;

pub use editor::Editor;
pub use env::EditorEnv;

pub use editor::*;

pub fn user_is_idle() -> bool {
    !user_is_active()
}

pub fn user_is_active() -> bool {
    let p_input = crate::core::event::pending_input_event_count();
    let p_rdr = crate::core::event::pending_render_event_count();
    p_input > 0 || p_rdr > 0
}
