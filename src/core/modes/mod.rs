use std::any::Any;

pub mod core_mode;
pub mod dir_fetch;
pub mod dir_mode;

pub mod find_mode;
pub mod goto_line_mode;
pub mod hsplit_mode;
pub mod line_number;
pub mod open_doc;
pub mod status_mode;
pub mod title_bar_mode;

pub mod text_mode;
pub mod vscrollbar_mode;
pub mod vsplit_mode;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;

use crate::core::editor::InputStageActionMap;
use crate::core::view::View;

pub use core_mode::CoreMode;
pub use find_mode::FindMode;
pub use goto_line_mode::GotoLineMode;
pub use hsplit_mode::HsplitMode;
pub use line_number::LineNumberMode;
pub use open_doc::OpenDocMode;

pub use status_mode::StatusMode;
pub use title_bar_mode::TitleBarMode;

pub use text_mode::TextMode;
pub use vscrollbar_mode::VscrollbarMode;
pub use vsplit_mode::VsplitMode;

pub use dir_mode::DirMode;

use crate::core::buffer::Buffer;
use crate::core::buffer::BufferEvent;

use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

pub trait Mode {
    // Returns the mode name
    fn name(&self) -> &'static str;

    /// This function exposes the mode's input function (name, pointer)
    fn build_action_map<'m>(&'m self) -> InputStageActionMap<'static>;

    /// TODO(ceg): find a better way to get back mode ctx
    fn alloc_ctx(&self) -> Box<dyn Any>;

    /// This function MUST be called once per buffer
    /// It is used to allocate buffer's mode context/metadata
    fn configure_buffer(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _buffer: &mut Buffer<'static>,
    ) {
    }

    fn on_buffer_event(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _event: &BufferEvent,
        _src_view: &mut View<'static>,
    ) {
        dbg_println!(
            "(default) mode '{}' on_buffer_event: event {:?} IGNORE",
            self.name(),
            _event
        );
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
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _src: ViewEventSource,
        _dst: ViewEventDestination,
        _event: &ViewEvent,
        _src_view: &mut View<'static>,
        _parent: Option<&mut View<'static>>,
    ) {
        dbg_println!(
            "(default) mode '{}' on_view_event src: {:?} dst: {:?}, event {:?}",
            self.name(),
            _src,
            _dst,
            _event
        );
    }
}
