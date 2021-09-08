/*
    TODO(ceg): provide the goto-line,goto-offset functions

    We want:

    An Updated collection of metadata that tracks number of newlines in the document sub-blocks

    The root contains the total count of newlines

    NB: This is tied to the document implementation

                 (root)
                   |
               [ 3 + 6 ]
             /          \
        [ 1 + 2 ]     [ 2 + 4 ]
       /        \    /        \
    [1]        [2]  [2]       [4]

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

use std::cell::RefCell;
use std::rc::Rc;
use std::sync::Arc;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::document;
use crate::core::document::Document;
use crate::core::document::DocumentEvent;
use crate::core::document::DocumentEventCb;

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
    static ref DOC_METADATA_MAP: Arc<RwLock<HashMap<document::Id, RwLock<LineNumberDocumentMetaData>>>> =
        Arc::new(RwLock::new(HashMap::new()));

        // document::Id -> (doc, LineNumberDocumentMetaData)
}

struct LineNumberDocumentMetaData {
    cb_installed: bool,
    root_idx: Option<usize>,
}

impl LineNumberDocumentMetaData {
    pub fn new() -> Self {
        dbg_println!("LineNumberDocumentMetaData");
        LineNumberDocumentMetaData {
            cb_installed: false,
            root_idx: None,
        }
    }
}

struct LineNumberDocumentNodeMetaData {
    nl_count: u64,
    cr_count: u64,
    lf_count: u64,
}

impl LineNumberDocumentNodeMetaData {
    pub fn new() -> Self {
        dbg_println!("LineNumberDocumentNodeMetaData");
        LineNumberDocumentNodeMetaData {
            nl_count: 0,
            cr_count: 0,
            lf_count: 0,
        }
    }
}

pub struct LineNumberMode {
    // add common fields
    doc_subscription: usize,
}

impl LineNumberMode {
    pub fn new() -> Self {
        dbg_println!("LineNumberMode");
        LineNumberMode {
            doc_subscription: 0,
        }
    }
}

pub struct LineNumberModeContext {
    // add per view fields
    target_vid: view::Id,
}

struct LineNumberModeDocEventHandler {
    pub count: usize,
}

impl<'a> Mode for LineNumberMode {
    fn name(&self) -> &'static str {
        &"line-number-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        InputStageActionMap::new()
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc line-number-mode ctx");
        let ctx = LineNumberModeContext { target_vid: 0 };
        Box::new(ctx)
    }

    fn configure_document(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        doc: &mut Document<'static>,
    ) {
        // allocate document meta data
        let doc_id = doc.id;

        DOC_METADATA_MAP
            .as_ref()
            .write()
            .entry(doc_id)
            .or_insert(RwLock::new(LineNumberDocumentMetaData::new()));

        let meta = DOC_METADATA_MAP.as_ref().write();
        let meta = meta.get(&doc_id);
        let meta = meta.as_ref().unwrap().write();

        if !meta.cb_installed {
            let cb = Box::new(LineNumberModeDocEventHandler { count: 0 });

            self.doc_subscription = doc.register_subscriber(cb);
        }
    }

    fn configure_view(
        &mut self,
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
        event: &ViewEvent,
    ) {
        let src_view = editor.view_map.get(&src.id).unwrap().write();
        let mut dst_view = editor.view_map.get(&dst.id).unwrap().write();

        match event {
            ViewEvent::Subscribe => {
                dbg_println!(
                    "LINENUM on_view_event src: {:?} dst: {:?}, event {:?}",
                    src,
                    dst,
                    event
                );

                let mut mode_ctx =
                    dst_view.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");
                mode_ctx.target_vid = src.id;
            }
            _ => {}
        }
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

impl DocumentEventCb for LineNumberModeDocEventHandler {
    fn cb(&mut self, doc: &Document, event: &DocumentEvent) {
        self.count += 1;

        dbg_println!(
            "LineNumberModeDocEventHandler ev {:?} CB count = {}",
            event,
            self.count
        );

        match event {
            DocumentEvent::NodeIndexed { node_index } => {
                dbg_println!(
                    "TODO index node {} with target codec  {:?}",
                    node_index,
                    event
                );
            }

            DocumentEvent::NodeAdded { node_index } => {}

            DocumentEvent::NodeRemoved { node_index } => {}

            DocumentEvent::NodeChanged { node_index } => {}

            _ => {
                eprintln!("unhandled event {:?}", event);
            }
        }

        doc.show_root_node_bytes_stats();
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
        env.screen.clear();
        for e in self.line_offsets.iter() {
            let s = format!("@{}", e.0);
            for c in s.chars() {
                let mut cpi = CodepointInfo::new();
                cpi.displayed_cp = c;
                env.screen.push(cpi);
            }
            env.screen.select_next_line_index();
        }
    }

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {}
}
