// Copyright (c) Carl-Erwin Griffith

use std::any::Any;

use super::super::Mode;

use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::InputStageActionMap;
use crate::core::modes::text_mode::RawDataFilter;
use crate::core::modes::text_mode::ScreenFilter;
use crate::core::modes::text_mode::TabFilter;
use crate::core::modes::text_mode::TextCodecFilter;
use crate::core::modes::text_mode::Utf8Filter;
use crate::core::modes::text_mode::WordWrapFilter;
use crate::core::view::View;
use crate::dbg_println;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

pub struct StatusModeContext {}

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
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        // let ctx = view.mode_ctx_mut::<StatusModeContext>("status-mode");

        //
        let use_utf8_codec = true;
        let use_tabulation_exp = true;
        let use_word_wrap = true;

        // mandatory data reader
        view.compose_filters
            .borrow_mut()
            .push(Box::new(RawDataFilter::new()));
        //

        if use_utf8_codec {
            //
            // DEBUG codec error
            view.compose_filters
                .borrow_mut()
                .push(Box::new(Utf8Filter::new()));
        } else {
            view.compose_filters
                .borrow_mut()
                .push(Box::new(TextCodecFilter::new()));
        }

        if use_tabulation_exp {
            view.compose_filters
                .borrow_mut()
                .push(Box::new(TabFilter::new()));
        }

        if use_word_wrap {
            // NB: Word Wrap after tab expansion
            view.compose_filters
                .borrow_mut()
                .push(Box::new(WordWrapFilter::new()));
        }

        let mut screen_filter = ScreenFilter::new();
        screen_filter.display_eof = false;

        view.compose_filters
            .borrow_mut()
            .push(Box::new(screen_filter));
    }
}

pub struct StatusMode {
    // add common filed
}

impl StatusMode {
    pub fn new() -> Self {
        dbg_println!("StatusMode");
        StatusMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}
