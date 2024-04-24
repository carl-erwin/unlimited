use parking_lot::RwLock;
use std::any::Any;
use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::editor::check_view_by_id;
use crate::core::editor::InputStageActionMap;

use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view;
use crate::core::view::ContentFilter;

use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::view::View;

pub struct TitleBarMode {
    // add common fields
}
pub struct TitleBarModeContext {
    // add per view fields
}

impl<'a> Mode for TitleBarMode {
    fn name(&self) -> &'static str {
        &"title-bar"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        InputStageActionMap::new()
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc TitleBarMode-mode ctx");
        let ctx = TitleBarModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        view.compose_content_filters
            .borrow_mut()
            .push(Box::new(EditorTitle::new()));
    }
}

impl TitleBarMode {
    pub fn new() -> Self {
        dbg_println!("TitleBarMode");
        TitleBarMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

///////////////////////////////////////////////////////////////////////////////////////////////////

struct EditorTitle {
    title: String,
    width: usize,
    height: usize,
}

impl EditorTitle {
    pub fn new() -> Self {
        EditorTitle {
            title: String::new(),
            width: 0,
            height: 0,
        }
    }
}

use crate::core::VERSION;

impl ContentFilter<'_> for EditorTitle {
    fn name(&self) -> &'static str {
        &"editor-title"
    }

    fn setup(
        &mut self,
        editor: &Editor<'static>,
        env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.width = env.screen.width();
        self.height = env.screen.height();

        let mut w = self.width;
        self.title = format!("unlimitED {} ", VERSION);
        w = w.saturating_sub(self.title.len());

        let view = view.read();
        let d = view.buffer().unwrap();
        let d = d.read();
        let mut buffer_info = format!("{}", d.name);
        let dlen = buffer_info.len();
        if w > dlen {
            let margin = 1; // w / 2 - dlen / 2;
            let margin = (0..margin).map(|_| " ").collect::<String>();
            self.title.push_str(&margin);
        }

        if d.changed {
            buffer_info.push_str("* ");
        } else {
            buffer_info.push_str("  ");
            //            buffer_info.push_str(" ❰❱❮❯  ◄ ► ▼ ▲");
        }
        if d.is_syncing {
            buffer_info.push_str("(sync)");
        }

        buffer_info.push_str(&format!(" {} bytes", d.size()));
        buffer_info.push_str(&format!(" (F1 for help)"));

        buffer_info.push_str(&format!(" (active vid: {:?})", env.active_view_id));

        /*
        if env.target_view_id != view::Id(0) {
            if let Some(_v) = check_view_by_id(editor, env.target_view_id) {
                buffer_info.push_str(&format!(" (target vid: {:?})", env.target_view_id));
            }
        }
        */

        self.title.push_str(&buffer_info);
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _filter_in: &[FilterIo],
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let color = TextStyle::title_color();
        let bg_color = TextStyle::title_bg_color();

        let width = env.screen.width();
        let mut count = 0;
        for c in self.title.chars().take(width) {
            let mut cpi = CodepointInfo::new();
            cpi.displayed_cp = c;
            cpi.style.color = color;
            cpi.style.bg_color = bg_color;
            let (b, _) = env.screen.push(&cpi);
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
            let (b, _) = env.screen.push(&cpi);
            if !b {
                break;
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {}
}
