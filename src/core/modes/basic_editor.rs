use std::any::Any;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::editor::InputStageActionMap;
use crate::core::modes::core_mode::split_with_direction;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterIoData;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;

use crate::core::view::View;

pub struct BasicEditorMode {
    // add common fields
}
pub struct BasicEditorModeContext {
    // add per view fields
}

impl<'a> Mode for BasicEditorMode {
    fn name(&self) -> &'static str {
        &"basic-editor"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc BasicEditorMode-mode ctx");
        let ctx = BasicEditorModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &self,
        mut editor: &mut Editor<'static>,
        mut env: &mut EditorEnv<'static>,
        mut view: &mut View<'static>,
    ) {
        let doc = view.document.clone();

        // children_layout_and_modes
        let ops_modes = vec![
            (
                LayoutOperation::Fixed {
                    size: 1 + 0, /* nano-like */
                },
                doc.clone(),
                vec![],
            ),
            (
                LayoutOperation::RemainMinus { minus: 3 },
                doc.clone(),
                vec!["core-mode".to_owned(), "text-mode".to_owned()],
            ),
            (LayoutOperation::Fixed { size: 3 }, None, vec![]),
        ];

        view.layout_direction = LayoutDirection::Horizontal;
        view.layout_ops = ops_modes.iter().map(|e| e.0.clone()).collect();
        let docs = ops_modes.iter().map(|e| e.1.clone()).collect();
        let modes = ops_modes.iter().map(|e| e.2.clone()).collect();

        let (width, height) = view.dimension();
        dbg_println!("width {}  height {}", width, height);

        split_with_direction(
            &mut editor,
            &mut env,
            &mut view,
            width,
            height,
            LayoutDirection::Horizontal,
            &docs,
            &modes,
        );

        let title_vid = view.children[0];
        let v = editor.view_map.get(&title_vid).unwrap();
        v.borrow_mut()
            .compose_filters
            .borrow_mut()
            .push(Box::new(BasicEditorTitle::new()));

        view.focus_to = Some(view.children[1]); // TODO: get focus
        env.focus_changed_to = Some(view.children[1]); // TODO:

        let status_vid = view.children[2];
        let v = editor.view_map.get(&status_vid).unwrap();
        v.borrow_mut()
            .compose_filters
            .borrow_mut()
            .push(Box::new(BasicEditorStatus::new()));
    }
}

impl BasicEditorMode {
    pub fn new() -> Self {
        dbg_println!("BasicEditorMode");
        BasicEditorMode {}
    }

    pub fn register_input_stage_actions<'a>(_map: &'a mut InputStageActionMap<'a>) {}
}

///////////////////////////////////////////////////////////////////////////////////////////////////

struct BasicEditorTitle {
    title: String,
    width: usize,
    height: usize,
}

impl BasicEditorTitle {
    pub fn new() -> Self {
        BasicEditorTitle {
            title: String::new(),
            width: 0,
            height: 0,
        }
    }
}

use crate::core::VERSION;

impl Filter<'_> for BasicEditorTitle {
    fn name(&self) -> &'static str {
        &"editor-title"
    }

    fn setup(&mut self, env: &LayoutEnv, view: &View) {
        self.width = env.screen.width();
        self.height = env.screen.height();

        let mut w = self.width;
        self.title = format!("unlimitED {} ", VERSION);
        w = w.saturating_sub(self.title.len());

        let d = view.document.as_ref().unwrap().read().unwrap();
        let mut doc_info = format!("{}", d.name);
        let dlen = doc_info.len();
        if w > dlen {
            let margin = 1; // w / 2 - dlen / 2;
            let margin = (0..margin).map(|_| " ").collect::<String>();
            self.title.push_str(&margin);
        }

        if d.changed {
            doc_info.push_str("* ");
        } else {
            doc_info.push_str("  ");
        }
        doc_info.push_str(&format!(" size {:<12}", d.size()));

        self.title.push_str(&doc_info);
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _filter_in: &Vec<FilterIoData>,
        _filter_out: &mut Vec<FilterIoData>,
    ) {
        let _bg_color = (100, 123, 153);

        let len = self.title.len();
        for c in self.title.chars() {
            let mut cpi = CodepointInfo::new();
            cpi.displayed_cp = c;
            cpi.metadata = true;
            cpi.size = 0;
            cpi.color = CodepointInfo::default_bg_color();
            cpi.bg_color = CodepointInfo::default_color();
            //            cpi.bg_color = (100, 123, 153);
            let (b, _) = env.screen.push(cpi.clone());
            if b == false {
                break;
            }
        }

        if len >= self.width {
            env.quit = true;
            return;
        }
        let remain = self.width - len;

        let _fill = ' ' as char;
        for _i in 0..remain {
            let mut cpi = CodepointInfo::new();
            cpi.color = CodepointInfo::default_bg_color();
            cpi.bg_color = CodepointInfo::default_color();
            cpi.metadata = true;

            let (b, _) = env.screen.push(cpi.clone());
            if b == false {
                env.quit = true;
                return;
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {}
}

struct BasicEditorStatus {}

impl BasicEditorStatus {
    pub fn new() -> Self {
        BasicEditorStatus {}
    }
}

impl Filter<'_> for BasicEditorStatus {
    fn name(&self) -> &'static str {
        &"editor-status"
    }

    fn setup(&mut self, _env: &LayoutEnv, _view: &View) {}

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _filter_in: &Vec<FilterIoData>,
        _filter_out: &mut Vec<FilterIoData>,
    ) {
        let fill = ' ' as char;
        loop {
            let mut cpi = CodepointInfo::new();
            cpi.displayed_cp = fill;
            cpi.metadata = true;
            cpi.color = CodepointInfo::default_bg_color();
            cpi.bg_color = CodepointInfo::default_color();

            let (b, _) = env.screen.push(cpi.clone());
            if b == false {
                break;
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) -> () {}
}
