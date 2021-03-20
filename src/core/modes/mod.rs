// Copyright (c) Carl-Erwin Griffith
use std::any::Any;

pub mod basic_editor;
pub mod core_mode;
pub mod hsplit_mode;
pub mod mode_template;
pub mod status_mode;
pub mod text_mode;
pub mod vsplit_mode;

pub use crate::core::editor::Editor;
pub use crate::core::editor::EditorEnv;

pub use crate::core::editor::InputStageActionMap;
pub use crate::core::view::View;

pub use basic_editor::BasicEditorMode;
pub use core_mode::CoreMode;
pub use hsplit_mode::HsplitMode;
pub use status_mode::StatusMode;
pub use text_mode::TextMode;
pub use vsplit_mode::VsplitMode;

pub trait Mode {
    // Returns the mode name
    fn name(&self) -> &'static str;
    /// This function exposes the mode's function (name, pointer)
    fn build_action_map<'m>(&'m self) -> InputStageActionMap<'static>;

    fn alloc_ctx(&self) -> Box<dyn Any>;

    fn configure_view(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _view: &mut View<'static>,
    );
}
