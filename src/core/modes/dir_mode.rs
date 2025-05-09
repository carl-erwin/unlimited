use std::any::Any;

use super::Mode;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::InputStageActionMap;

use crate::core::modes::text_mode::ScreenFilter;

use crate::core::view::View;

use crate::core::screen::screen_apply;

use crate::dbg_println;

use crate::core::view::ContentFilter;

use crate::core::view::LayoutEnv;

use crate::core::codepointinfo::TextStyle;

use crate::core::modes::dir_fetch::DirectoryReader;

pub struct DirModeContext {}

pub struct DirMode {}

impl DirMode {
    pub fn new() -> Self {
        dbg_println!("DirMode");
        DirMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl<'a> Mode for DirMode {
    fn name(&self) -> &'static str {
        &"dir-mode"
    }

    fn alloc_ctx(&self, _editor: &Editor) -> Box<dyn Any> {
        let ctx = DirModeContext {};
        Box::new(ctx)
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        // let ctx = view.mode_ctx_mut::<DirModeContext>("dir-mode");

        // mandatory directory reader
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(DirectoryReader::new()));

        let mut screen_filter = ScreenFilter::new();
        screen_filter.display_eof = false;

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(screen_filter));
    }
}

pub struct DirModeCompose {
    // add common fields
}

impl DirModeCompose {
    pub fn new() -> Self {
        dbg_println!("DirMode");
        DirModeCompose {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl ContentFilter<'_> for DirModeCompose {
    fn name(&self) -> &'static str {
        &"DirModeCompose"
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        // fill the whole dir bar
        screen_apply(&mut env.screen, |_, _, cpi| {
            cpi.style.color = TextStyle::title_color();
            cpi.style.bg_color = TextStyle::title_bg_color();

            true
        });
    }
}
