// Copyright (c) Carl-Erwin Griffith
use std::any::Any;

pub mod text_mode;

pub use crate::core::editor::InputStageActionMap;
pub use crate::core::view::View;

pub use text_mode::TextMode; // TODO remove

pub trait Mode {
    // Returns the mode name
    fn name(&self) -> &'static str;
    /// This function exposes the mode's function (name, pointer)
    fn build_action_map<'m>(&'m self) -> InputStageActionMap<'static>;
    //    fn build_render_map() -> RenderMap;

    fn alloc_ctx(&self) -> Box<dyn Any>;
    fn configure_view(&self, view: &mut View);
}
