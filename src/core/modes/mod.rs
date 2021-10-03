use std::any::Any;

pub mod basic_editor;
pub mod core_mode;
pub mod find_mode;
pub mod hsplit_mode;
pub mod line_number;
pub mod mode_template;
pub mod simple_view;
pub mod status_mode;
pub mod text_mode;
pub mod vscrollbar_mode;
pub mod vsplit_mode;

pub use crate::core::editor::Editor;
pub use crate::core::editor::EditorEnv;

pub use crate::core::editor::InputStageActionMap;
pub use crate::core::view::View;

pub use basic_editor::BasicEditorMode;
pub use core_mode::CoreMode;
pub use find_mode::FindMode;
pub use hsplit_mode::HsplitMode;
pub use line_number::LineNumberMode;
pub use simple_view::SimpleViewMode;
pub use status_mode::StatusMode;
pub use text_mode::TextMode;
pub use vscrollbar_mode::VscrollbarMode;
pub use vsplit_mode::VsplitMode;

use crate::core::document::Document;

use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

pub trait Mode {
    // Returns the mode name
    fn name(&self) -> &'static str;

    /// This function exposes the mode's input function (name, pointer)
    fn build_action_map<'m>(&'m self) -> InputStageActionMap<'static>;

    fn alloc_ctx(&self) -> Box<dyn Any>;

    /// This function MUST be called once per document
    /// It is used to allocate document's mode context/metadata
    fn configure_document(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _doc: &mut Document<'static>,
    ) {
    }

    /// This function MUST be called once per view
    /// It is used to allocate view's mode context
    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _view: &mut View<'static>,
    );

    fn on_view_event(
        &self,
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        _src: ViewEventSource,
        _dst: ViewEventDestination,
        _event: &ViewEvent,
        _parent: Option<&mut View<'static>>,
    ) {
    }
}