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

use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::editor::check_view_by_id;

use crate::core::view;
use crate::core::view::ContentFilter;

use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::dbg_println;

pub struct StatusLineModeContext {}

pub struct StatusLineMode {}

impl StatusLineMode {
    pub fn new() -> Self {
        dbg_println!("StatusLineMode");
        StatusLineMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl<'a> Mode for StatusLineMode {
    fn name(&self) -> &'static str {
        &"status-line-mode"
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        let ctx = StatusLineModeContext {};
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
        // let ctx = view.mode_ctx_mut::<StatusLineModeContext>(self.name());

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
            .push(Box::new(StatusLineModeCompose::new()));
    }
}

pub struct StatusLineModeCompose {
    // add common fields
    width: usize,
    height: usize,
    content: String,
}

impl StatusLineModeCompose {
    pub fn new() -> Self {
        dbg_println!("StatusLineMode");
        StatusLineModeCompose {
            width: 0,
            height: 0,
            content: String::new(),
        }
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl ContentFilter<'_> for StatusLineModeCompose {
    fn name(&self) -> &'static str {
        &"status-line"
    }

    fn setup(
        &mut self,
        editor: &Editor<'static>,
        env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.width = env.screen.width();
        self.height = env.screen.height();

        //let mut w = self.width;
        self.content.clear();

        // w = w.saturating_sub(self.content.len());

        let mut buffer_info = String::new();

        // TODO: add target_view == text-view  != controller

        if env.target_view_id != view::Id(0) {
            if let Some(f) = check_view_by_id(editor, env.target_view_id) {
                let f = f.read();
                let b = f.buffer().unwrap();
                let n = &b.read().name;
                buffer_info.push_str(&format!("[{}]", n));
            }
        }

        self.content.push_str(&buffer_info);
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _filter_in: &[FilterIo],
        _filter_out: &mut Vec<FilterIo>,
    ) {
        env.screen.clear();

        let color = TextStyle::title_color();
        let bg_color = TextStyle::title_bg_color();

        let width = env.screen.width();
        let mut count = 0;
        for c in self.content.chars().take(width) {
            let mut cpi = CodepointInfo::new();
            cpi.displayed_cp = c;
            cpi.style.color = color;
            cpi.style.bg_color = bg_color;
            let (b, _) = env.screen.push(cpi.clone());
            if !b {
                break;
            }
            count += 1;
        }

        let _fill = ' ' as char;
        let mut cpi = CodepointInfo::new();
        cpi.style.color = color;
        cpi.style.bg_color = bg_color;
        for _i in count..width {
            let (b, _) = env.screen.push(cpi.clone());
            if !b {
                break;
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {}
}
