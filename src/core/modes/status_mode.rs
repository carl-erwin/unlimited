use std::any::Any;

use super::Mode;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::InputStageActionMap;
use crate::core::modes::text_mode::RawDataFilter;
use crate::core::modes::text_mode::ScreenFilter;
use crate::core::modes::text_mode::TabFilter;
use crate::core::modes::text_mode::TextCodecFilter;
use crate::core::modes::text_mode::UnicodeToTextFilter;
use crate::core::modes::text_mode::Utf8Filter;
use crate::core::view::View;

use crate::core::screen::screen_apply;

use crate::dbg_println;

use crate::core::view::ContentFilter;

use crate::core::view::LayoutEnv;

use crate::core::codepointinfo::TextStyle;

pub struct StatusModeContext {}

pub struct StatusMode {}

impl StatusMode {
    pub fn new() -> Self {
        dbg_println!("StatusMode");
        StatusMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl<'a> Mode for StatusMode {
    fn name(&self) -> &'static str {
        &"status-mode"
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        let ctx = StatusModeContext {};
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
        // let ctx = view.mode_ctx_mut::<StatusModeContext>("status-mode");

        //
        let use_utf8_codec = true;
        let use_tabulation_exp = true;

        // mandatory data reader
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(RawDataFilter::new()));
        //

        if use_utf8_codec {
            //
            // DEBUG codec error
            view.compose_content_filters
                .borrow_mut()
                .push(Box::new(Utf8Filter::new()));
        } else {
            view.compose_content_filters
                .borrow_mut()
                .push(Box::new(TextCodecFilter::new()));
        }
        //
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(UnicodeToTextFilter::new()));

        // TODO: char map 0x9 -> "\t"
        if use_tabulation_exp {
            view.compose_content_filters
                .borrow_mut()
                .push(Box::new(TabFilter::new()));
        }

        let mut screen_filter = ScreenFilter::new();
        screen_filter.display_eof = false;

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(screen_filter));

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(StatusModeCompose::new()));
    }
}

pub struct StatusModeCompose {
    // add common fields
}

impl StatusModeCompose {
    pub fn new() -> Self {
        dbg_println!("StatusMode");
        StatusModeCompose {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl ContentFilter<'_> for StatusModeCompose {
    fn name(&self) -> &'static str {
        &"StatusModeCompose"
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        // fill the whole status bar
        screen_apply(&mut env.screen, |_, _, cpi| {
            cpi.style.color = TextStyle::title_color();
            cpi.style.bg_color = TextStyle::title_bg_color();

            true
        });
    }
}
