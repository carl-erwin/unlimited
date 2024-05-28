use std::any::Any;

use parking_lot::RwLock;
use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::editor::InputStageActionMap;
use crate::core::modes::text_mode::RawDataFilter;
use crate::core::modes::text_mode::ScreenFilter;
use crate::core::view::View;

use crate::dbg_println;

use crate::core::view::ContentFilter;

use crate::core::view::LayoutEnv;

pub struct SideBarModeContext {}

pub struct SideBarMode {}

impl SideBarMode {
    pub fn new() -> Self {
        dbg_println!("SideBarMode");
        SideBarMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl<'a> Mode for SideBarMode {
    fn name(&self) -> &'static str {
        &"side-bar-mode"
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        let ctx = SideBarModeContext {};
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
        // mandatory data reader
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(RawDataFilter::new()));

        /*
            if use_utf8_codec {
                /* DEBUG codec error */
                view.compose_content_filters
                    .borrow_mut()
                    .push(Box::new(Utf8Filter::new()));
            }

            view.compose_content_filters
            .borrow_mut()
            .push(Box::new(UnicodeToTextFilter::new()));

        */

        let mut screen_filter = ScreenFilter::new();
        screen_filter.display_eof = false;

        /*
            view.compose_content_filters
            .borrow_mut()
            .push(Box::new(screen_filter));
        */

        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(SideBarModeCompose::new()));
    }
}

pub struct SideBarModeCompose {
    // add common fields
    buffer_list: Vec<String>,
}

impl SideBarModeCompose {
    pub fn new() -> Self {
        dbg_println!("SideBarMode");
        SideBarModeCompose {
            buffer_list: vec![],
        }
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

impl ContentFilter<'_> for SideBarModeCompose {
    fn name(&self) -> &'static str {
        &"SideBarModeCompose"
    }

    fn setup(
        &mut self,
        editor: &mut Editor<'static>,
        _editor_env: &mut EditorEnv<'static>,

        _env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        // TODO(ceg): cache list
        self.buffer_list.clear();

        // get editor document list
        for (id, b) in editor.buffer_map.read().iter() {
            let b = b.read();
            self.buffer_list.push(b.name.clone());
        }

        // order by cmd-line -> sort by buffer id order
        // order by name
        self.buffer_list.sort();
    }

    fn finish(&mut self, _view: &View, env: &mut LayoutEnv) {
        // fill the whole status bar

        // TODO(ceg): add screen.println("")
        // TODO(ceg): add screen.println_truncate("")

        env.screen.clear();
        let w = env.screen.width().saturating_sub(0);
        for s in &self.buffer_list {
            let slen = s.len();
            for c in s.chars().take(w) {
                let mut cpi = CodepointInfo::new();
                cpi.displayed_cp = c;
                cpi.cp = c;
                env.screen.push(&cpi);
                // push no newline
            }
            if slen < w {
                env.screen.select_next_line_index();
            }
        }
    }
}
