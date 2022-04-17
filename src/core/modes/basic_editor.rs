use parking_lot::RwLock;
use std::any::Any;
use std::rc::Rc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use crate::core::editor::InputStageActionMap;
use crate::core::modes::core_mode::split_with_direction;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::view::ContentFilter;

use crate::core::view::FilterIo;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutEnv;
use crate::core::view::LayoutOperation;

use crate::core::view::View;

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
        &mut self,
        mut editor: &mut Editor<'static>,
        mut env: &mut EditorEnv<'static>,
        mut view: &mut View<'static>,
    ) {
        let doc = view.document();

        let status_doc = DocumentBuilder::new()
            .document_name("")
            .internal(true)
            //           .use_buffer_log(false)
            .finalize();

        let status_doc = status_doc;

        // hsplit

        // children_layout_and_modes
        let ops_modes = vec![
            (
                LayoutOperation::Fixed {
                    size: 1 + 0, /* nano-like */
                },
                doc.clone(),
                vec![], // TODO(ceg): title-mode
            ),
            (
                LayoutOperation::RemainMinus { minus: 2 },
                doc.clone(),
                vec!["simple-view".to_owned()],
            ),
            (
                LayoutOperation::Fixed { size: 1 },
                None,
                vec!["hsplit-mode".to_owned()],
            ),
            (
                LayoutOperation::RemainPercent { p: 100.0 },
                status_doc,
                vec!["status-mode".to_owned()],
            ),
        ];

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
            LayoutDirection::Vertical,
            &docs,
            &modes,
        );

        // mark children as non destroyable
        for i in 0..view.children.len() {
            let vid = view.children[i];
            let v = editor.view_map.get(&vid).unwrap();
            v.write().destroyable = false;
        }

        // TODO(ceg): put some kind of label/name? on
        // like view.label = 'text-view'
        // like view.label = 'status-line'
        // view.children_by_label<String, (vid, index)>

        // set focus on text view : TODO(ceg): title mode + configure
        let title_vid = view.children[0];
        let v = editor.view_map.get(&title_vid).unwrap();
        v.write()
            .compose_content_filters
            .borrow_mut()
            .push(Box::new(BasicEditorTitle::new()));

        // set focus on text view (simple-view mode)
        view.main_child = Some(1); // index in children
        view.focus_to = Some(view.children[1]); // TODO(ceg):
        env.focus_changed_to = Some(view.children[1]); // TODO(ceg):

        // TODO(ceg): status mode + configure
        // setup status view
        let status_vid = view.children[view.children.len() - 1];
        // set status_vid
        view.status_view_id = Some(status_vid);
        env.status_view_id = Some(status_vid);
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

impl ContentFilter<'_> for BasicEditorTitle {
    fn name(&self) -> &'static str {
        &"editor-title"
    }

    fn setup(
        &mut self,
        editor: &Editor,
        env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        parent_view: Option<&View<'static>>,
    ) {
        self.width = env.screen.width();
        self.height = env.screen.height();

        let mut w = self.width;
        self.title = format!("unlimitED {} ", VERSION);
        w = w.saturating_sub(self.title.len());

        let view = view.read();
        let d = view.document().unwrap();
        let d = d.read();
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
        if d.is_syncing {
            doc_info.push_str(" (sync) ");
        }

        if true {
            let mut have_offset = false;
            let mut view_start_offset = 0;
            let mut view_end_offset = 0;

            if let Some(parent) = parent_view {
                if let Some(focus) = parent.focus_to {
                    if let Some(v) = editor.view_map.get(&focus) {
                        let v = &v.read();
                        view_start_offset = v.start_offset;
                        view_end_offset = v.end_offset;

                        have_offset = true;
                    }
                }
            }

            if have_offset {
                doc_info.push_str(&format!(
                    " {}-{}/{}",
                    view_start_offset,
                    view_end_offset,
                    d.size()
                ));
            } else {
                doc_info.push_str(&format!(" size {:<12}", d.size()));
                doc_info.push_str(&format!(" unknown @offset"));
            }
        }

        {
            let p_input = crate::core::event::pending_input_event_count();
            let p_rdr = crate::core::event::pending_render_event_count();
            doc_info.push_str(&format!(" pending input:{} rdr:{}", p_input, p_rdr));
        }

        self.title.push_str(&doc_info);
    }

    fn run(
        &mut self,
        _view: &View,
        env: &mut LayoutEnv,
        _filter_in: &Vec<FilterIo>,
        _filter_out: &mut Vec<FilterIo>,
    ) {
        let _bg_color = (113, 114, 123);

        let len = self.title.len();
        for c in self.title.chars() {
            let mut cpi = CodepointInfo::new();
            cpi.displayed_cp = c;
            cpi.style.color = TextStyle::default_bg_color();
            cpi.style.bg_color = TextStyle::default_color();
            let (b, _) = env.screen.push(cpi.clone());
            if !b {
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
            cpi.style.color = TextStyle::default_bg_color(); // remove ?
            cpi.style.bg_color = TextStyle::default_color(); // remove ?

            let (b, _) = env.screen.push(cpi.clone());
            if !b {
                env.quit = true;
                return;
            }
        }

        env.quit = true;
    }

    fn finish(&mut self, _view: &View, _env: &mut LayoutEnv) {}
}
