use std::any::Any;
pub mod text_mode;

pub use crate::core::editor::ActionMap;
pub use text_mode::TextMode;

pub trait Mode {
    // Returns the mode name
    fn name(&self) -> &'static str;
    /// This function exposes the mode's function (name, pointer)
    fn build_action_map(&self) -> ActionMap;
    //    fn build_render_map() -> RenderMap;

    fn alloc_ctx(&self) -> Box<dyn Any>;
}

/*
   [Mode] create_context() -> Box<dyn Any>

   [view] . set_mode_context("mode-name", Box<dyn Any>) -> bool
                          mode_ctx_context(""mode-name")              -> Box<dyn Any>)





*/
