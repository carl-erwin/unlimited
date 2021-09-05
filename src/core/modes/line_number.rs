/*
    TODO(ceg): provide the goto-line,goto-offset functions

    We want:

    An Updated collection of metadata that tracks number of newlines in the document sub-blocks

    The root contains the total count of newlines

    NB: This is tied to the document implementation

              (root)
            [  4 + 6 ]
          /           \
        [ 2 + 2 ]     [ 2 + 4 ]
       /        \    /        \
    [2]        [2]  [2]       [4]

    each time a node is indexed
    the document impl must call update hierarchy with the build metadata diff

    the mode subscribe to the document events

    When a node is indexed/added/removed, the document notify us
    Then we build the node metadata
    and ask to update the hierarchy.


    To be fully async we must: have a shadowed tree that matched the document internal representation ?
    and keep a per node doc_revision

    Must we re-index before remove ?



*/

use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;
use std::sync::Arc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::document;
use crate::core::document::Document;

use crate::core::view::layout::LayoutEnv;
use crate::core::view::layout::ScreenOverlayFilter;

use crate::core::view;
use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use lazy_static::lazy_static;
use std::collections::HashMap;
// document meta data map
lazy_static! {
    static ref DOC_METADATA_MAP: Arc<RwLock<HashMap<document::Id, Arc<RwLock<Document<'static>>>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

struct LineNumberDocumentNodeMetaData {
    nl_count: u64,
    cr_count: u64,
    lf_count: u64,
}

impl LineNumberDocumentNodeMetaData {
    pub fn new() -> Self {
        dbg_println!("LineNumberMode");
        LineNumberDocumentNodeMetaData {
            nl_count: 0,
            cr_count: 0,
            lf_count: 0,
        }
    }
}

pub struct LineNumberMode {
    // add common fields
}

impl LineNumberMode {
    pub fn new() -> Self {
        dbg_println!("LineNumberMode");
        LineNumberMode {}
    }
}

pub struct LineNumberModeContext {
    // add per view fields
    target_vid: view::Id,
}

impl<'a> Mode for LineNumberMode {
    fn name(&self) -> &'static str {
        &"line-number-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc line-number-mode ctx");
        let ctx = LineNumberModeContext { target_vid: 0 };
        Box::new(ctx)
    }

    fn configure_view(
        &self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        view.compose_screen_overlay_filters
            .borrow_mut()
            .push(Box::new(LineNumberOverlayFilter::new()));
    }

    fn on_view_event(
        &self,
        editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        src: ViewEventSource,
        dst: ViewEventDestination,
        _event: &ViewEvent,
    ) {
        dbg_println!("LINENUM on_view_event src: {:?}", src);
        dbg_println!("LINENUM on_view_event dst: {:?}", dst);

        let src = editor.view_map.get(&src.id).unwrap().write();
        let mut dst = editor.view_map.get(&dst.id).unwrap().write();
        let mut mode_ctx = dst.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");
        mode_ctx.target_vid = src.id;

        let doc_id = src.document.as_ref().unwrap().read().id;
        let d = DOC_METADATA_MAP.as_ref().write().get_mut(&doc_id).unwrap();
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct LineNumberOverlayFilter {
    line_offsets: Vec<(u64, u64)>,
}

impl LineNumberOverlayFilter {
    pub fn new() -> Self {
        LineNumberOverlayFilter {
            line_offsets: vec![],
        }
    }
}

impl ScreenOverlayFilter<'_> for LineNumberOverlayFilter {
    fn name(&self) -> &'static str {
        &"LineNumberOverlay"
    }

    fn setup(&mut self, editor: &Editor, _env: &mut LayoutEnv, view: &Rc<RwLock<View>>) {
        let view = view.read();
        let mode_ctx = view.mode_ctx::<LineNumberModeContext>("line-number-mode");
        let target_vid = mode_ctx.target_vid;
        let src = editor.view_map.get(&target_vid).unwrap().read();
        self.line_offsets = src.screen.read().line_offset.clone();
    }

    fn run(&mut self, _view: &View, env: &mut LayoutEnv) -> () {
        dbg_println!("LINENUM RUN");
        env.screen.clear();
        for e in self.line_offsets.iter() {
            let s = format!("@{:>10}", e.0);
            for c in s.chars() {
                let mut cpi = CodepointInfo::new();
                cpi.displayed_cp = c;
                let ret = env.screen.push(cpi);
                dbg_println!("ret = {:?}", ret);
            }
            env.screen.select_next_line_index();
        }
    }

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {
        dbg_println!("LINENUM FINISH");
    }
}
