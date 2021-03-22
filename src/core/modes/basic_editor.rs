use std::any::Any;
use std::sync::Arc;
use std::sync::RwLock;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::editor::InputStageActionMap;
use crate::core::modes::core_mode::split_with_direction;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::layout::Filter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;

use crate::core::screen::Screen;
use crate::core::view::View;

use crate::core::modes::text_mode::ScreenFilter;
use crate::core::modes::text_mode::TabFilter;
use crate::core::modes::text_mode::WordWrapFilter;
use crate::core::modes::HsplitMode;

use crate::core::document::DocumentBuilder;

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
        let doc = view.document();

        let status_doc = DocumentBuilder::new()
            .document_name("")
            .file_name("/dev/null")
            .internal(true)
            //           .use_buffer_log(false)
            .finalize();

        let status_doc = status_doc;

        // hsplt

        // children_layout_and_modes
        let ops_modes = vec![
            (
                LayoutOperation::Fixed {
                    size: 1 + 0, /* nano-like */
                },
                doc.clone(),
                vec![], // TODO: title-mode
            ),
            (
                LayoutOperation::RemainMinus { minus: 2 },
                doc.clone(),
                vec!["core-mode".to_owned(), "text-mode".to_owned()],
            ),
            (
                LayoutOperation::Fixed { size: 1 },
                None,
                vec!["hsplit-mode".to_owned()],
            ),
            (
                LayoutOperation::RemainPercent { p: 100 },
                status_doc,
                vec!["status-mode".to_owned()],
            ),
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

        // mark children as non destroyable
        for i in 0..view.children.len() {
            let vid = view.children[i];
            let v = editor.view_map.get(&vid).unwrap();
            v.borrow_mut().destroyable = false;
        }


        // TODO: put some kind of label on
        // like view.label = 'text-view'
        // like view.label = 'status-line'
        // view.children_by_label<String, (vid, index)>

        // set focus on text view : TODO: title mode + configure
        let title_vid = view.children[0];
        let v = editor.view_map.get(&title_vid).unwrap();
        v.borrow_mut()
            .compose_filters
            .borrow_mut()
            .push(Box::new(BasicEditorTitle::new()));

        // set focus on text view
        view.focus_to = Some(view.children[1]); // TODO: get focus
        env.focus_changed_to = Some(view.children[1]); // TODO:

        // TODO: status mode + configure
        // setup status view
        let status_vid = view.children[view.children.len() - 1];
        // set status_vid
        view.status_view_id = Some(status_vid);
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

    fn setup(&mut self, env: &mut LayoutEnv, view: &View) {
        self.width = env.screen.width();
        self.height = env.screen.height();

        let mut w = self.width;
        self.title = format!("unlimitED {} ", VERSION);
        w = w.saturating_sub(self.title.len());

        let d = view.document().unwrap();
        let d = d.read().unwrap();
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
        _filter_in: &Vec<FilterIo>,
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let _bg_color = (100, 123, 153);

        let len = self.title.len();
        for c in self.title.chars() {
            let mut cpi = CodepointInfo::new();
            cpi.displayed_cp = c;
            cpi.style.color = TextStyle::default_bg_color();
            cpi.style.bg_color = TextStyle::default_color();
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
            cpi.style.color = TextStyle::default_bg_color();
            cpi.style.bg_color = TextStyle::default_color();

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
