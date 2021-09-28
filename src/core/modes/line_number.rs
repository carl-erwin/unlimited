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

use crate::core::mapped_file::MappedFile;
use crate::core::mapped_file::MappedFileIterator;

use crate::core::view::layout::LayoutEnv;
use crate::core::view::layout::ScreenOverlayFilter;

use crate::core::view;
use crate::core::view::LayoutOperation;
use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use lazy_static::lazy_static;
use std::collections::HashMap;

fn num_digit(v: usize) -> usize {
    match v {
        _ if v < 10 => 1,
        _ if v < 100 => 2,
        _ if v < 1000 => 3,
        _ if v < 10000 => 4,
        _ if v < 100000 => 5,
        _ if v < 1000000 => 6,
        _ if v < 10000000 => 7,
        _ if v < 100000000 => 8,
        _ if v < 1000000000 => 9,
        _ if v < 10000000000 => 10,
        _ if v < 100000000000 => 11,
        _ if v < 1000000000000 => 12,
        _ if v < 10000000000000 => 13,
        _ if v < 100000000000000 => 14,
        _ if v < 1000000000000000 => 15,
        _ if v < 10000000000000000 => 16,
        _ if v < 100000000000000000 => 17,
        _ if v < 1000000000000000000 => 18,
        _ if v < 10000000000000000000 => 19,
        _ => 20,
    }
}

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
        parent: Option<&mut View<'static>>,
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

            ViewEvent::PreComposition => {
                // TODO(ceg): resize line-number view
                let doc = src_view.document();
                let doc = doc.as_ref().unwrap().read();
                let max_offset = doc.size() as u64 + 1;
                let width = if !doc.indexed {
                    // '@offset '
                    1 + num_digit(max_offset as usize) + 1
                } else {
                    let ret = get_byte_count_at_offset(&doc, '\n' as usize, max_offset);
                    let n = num_digit(ret.0 as usize + 1); // nb line = line count + 1

                    // 'xxxx '
                    n + 1
                };

                if let Some(p_view) = parent {
                    p_view.layout_ops[dst_view.layout_index.unwrap()] =
                        LayoutOperation::Fixed { size: width };
                } else {
                    panic!("");
                }

                // TODO store width
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
                dbg_println!("unhandled event {:?}", event);
            }
        }

        doc.show_root_node_bytes_stats();
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

pub struct LineNumberOverlayFilter {
    line_offsets: Vec<(u64, u64)>,
    line_number: Vec<(u64, (u64, Option<usize>))>, // (offset, (line_num, node_index))
}

impl LineNumberOverlayFilter {
    pub fn new() -> Self {
        LineNumberOverlayFilter {
            line_offsets: vec![],
            line_number: vec![],
        }
    }
}

// TODO(ceg): move to document.rs
//
// walk through the binary tree and while looking for the node containing "offset"
// and track byte_index count
//                                   SZ(19)   ,       LF(9)
//                   _________[ SZ(7+12),  LF(3+6) ]____________________
//                  /                                                 \
//        __[ 7=SZ(3+4), LF 3=(1+2) ]__                        _____[ 12=(5+7),  LF 6=(2+4) ]__
//       /                             \                      /                                 \
//  [SZ(3), LF(1)]={a,LF,b}    [SZ(4), LF(2)]={a,LF,LF,b }   [5, LF(2)] data{a,LF,b,LF,c} [SZ(7), LF(4)]={a ,LF,LF,b ,Lf,LF,c}
//                  0,1 ,2                     3, 4, 5,6                     7, 8,9,10,11                 12,13,14,15,16,17,18
//
//
// return (line_count, offset's node_index)
fn get_byte_count_at_offset(
    doc: &Document,
    byte_index: usize,
    offset: u64,
) -> (u64, Option<usize>) {
    assert!(byte_index < 256);

    let mut file = doc.buffer.data.as_ref().write();
    let mut cur_index = file.root_index();
    let mut total_count = 0;
    let mut local_offset = offset;
    while cur_index != None {
        let idx = cur_index.unwrap();
        let p_node = &file.pool[idx];

        let is_leaf = p_node.link.left.is_none() && p_node.link.right.is_none();
        if is_leaf {
            let data = document::get_node_data(&mut file, Some(idx));
            for b in data.iter().take(local_offset as usize) {
                if *b as usize == byte_index {
                    total_count += 1;
                }
            }
            return (total_count, cur_index);
        }

        let mut left_node_size = 0;
        let mut left_byte_count = 0;

        if let Some(left_index) = p_node.link.left {
            let left_node = &file.pool[left_index];

            if local_offset < left_node.size {
                cur_index = Some(left_index);
                continue;
            }

            left_byte_count = left_node.byte_count[byte_index];
            left_node_size = left_node.size;
        }

        if let Some(right_index) = p_node.link.right {
            total_count += left_byte_count;
            local_offset -= left_node_size;
            cur_index = Some(right_index);
            continue;
        }
    }

    (0, None)
}

impl ScreenOverlayFilter<'_> for LineNumberOverlayFilter {
    fn name(&self) -> &'static str {
        &"LineNumberOverlay"
    }

    fn setup(
        &mut self,
        editor: &Editor,
        _env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        let view = view.read();
        let mode_ctx = view.mode_ctx::<LineNumberModeContext>("line-number-mode");
        let target_vid = mode_ctx.target_vid;
        let src = editor.view_map.get(&target_vid).unwrap().read();
        self.line_offsets = src.screen.read().line_offset.clone();

        self.line_number.clear();

        let doc = src.document();
        let doc = doc.as_ref().unwrap().read();
        if !doc.indexed {
            return;
        }

        // call to get_byte_count_at_offset are SLOW : compute only the first line
        // and read the target screen to compute relative line count
        for offset in self.line_offsets.iter().take(1) {
            let n = get_byte_count_at_offset(&doc, '\n' as usize, offset.0);
            self.line_number.push((offset.0, n));
        }

        let mut line_number = self.line_number[0].1 .0;
        let screen = src.screen.as_ref().read();
        for i in 0..screen.line_index.len() {
            let mut offset = 0;
            if let Some(l) = screen.get_used_line(i) {
                for (idx, cell) in l.iter().enumerate() {
                    if idx == 0 {
                        offset = cell.cpi.offset.unwrap();
                    }
                    if cell.cpi.metadata == false && cell.cpi.cp == '\n' {
                        line_number += 1;
                        break;
                    }
                }
            }
            self.line_number.push((offset, (line_number as u64, None)));
        }
    }

    fn run(&mut self, _view: &View, env: &mut LayoutEnv) -> () {
        env.screen.clear();

        let w = env.screen.width();
        if !self.line_number.is_empty() {
            let mut prev_line = 0;
            for (idx, e) in self.line_number.iter().enumerate() {
                let s = if idx > 0 && e.1 .0 == prev_line {
                    // clear line
                    format!("") // AFTER DEBUG ENABLE THIS
                } else {
                    format!("{}", e.1 .0 + 1)
                };
                prev_line = e.1 .0;

                let padding = w - s.len() - 1;
                for _ in 0..padding {
                    env.screen.push(CodepointInfo::new());
                }

                for c in s.chars() {
                    let mut cpi = CodepointInfo::new();
                    cpi.displayed_cp = c;
                    cpi.style.color.0 = cpi.style.color.0.saturating_sub(70);
                    cpi.style.color.1 = cpi.style.color.1.saturating_sub(70);
                    cpi.style.color.2 = cpi.style.color.2.saturating_sub(70);

                    env.screen.push(cpi);
                }
                env.screen.select_next_line_index();
            }
            return;
        }

        for e in self.line_offsets.iter() {
            let s = format!("@{}", e.0);
            for c in s.chars() {
                let mut cpi = CodepointInfo::new();
                cpi.displayed_cp = c;
                cpi.style.color.0 = cpi.style.color.0.saturating_sub(70);
                cpi.style.color.1 = cpi.style.color.1.saturating_sub(70);
                cpi.style.color.2 = cpi.style.color.2.saturating_sub(70);

                env.screen.push(cpi);
            }
            env.screen.select_next_line_index();
        }
    }

    fn finish(&mut self, view: &View, env: &mut LayoutEnv) -> () {}
}
