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

    the mode subscribes to the document events

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

use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_vid;
use crate::core::editor::InputStageActionMap;
use crate::core::event::input_map::build_input_event_map;

use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::document;
use crate::core::document::get_document_byte_count_at_offset;
use crate::core::document::Document;
use crate::core::document::DocumentEvent;
use crate::core::document::DocumentEventCb;

use crate::core::view::LayoutEnv;
use crate::core::view::ScreenOverlayFilter;

use crate::core::view;
use crate::core::view::LayoutOperation;
use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::event::*;

use lazy_static::lazy_static;
use std::collections::HashMap;

fn num_digit(v: u64) -> u64 {
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

static LINENUM_INPUT_MAP: &str = r#"
[
  {
    "events": [
     { "default": [],                    "action": "line-number:input-event" }
   ]
  }

]"#;

// document meta data map
lazy_static! {
    static ref DOC_METADATA_MAP: Arc<RwLock<HashMap<document::Id, RwLock<LineNumberDocumentMetaData>>>> =
        Arc::new(RwLock::new(HashMap::new()));

        // document::Id -> (doc, LineNumberDocumentMetaData)
}

struct LineNumberDocumentMetaData {
    cb_installed: bool,
    _root_idx: Option<usize>,
}

impl LineNumberDocumentMetaData {
    pub fn new() -> Self {
        dbg_println!("LineNumberDocumentMetaData");
        LineNumberDocumentMetaData {
            cb_installed: false,
            _root_idx: None,
        }
    }
}

struct _LineNumberDocumentNodeMetaData {
    nl_count: u64,
    cr_count: u64,
    lf_count: u64,
}

impl _LineNumberDocumentNodeMetaData {
    pub fn _new() -> Self {
        dbg_println!("LineNumberDocumentNodeMetaData");
        _LineNumberDocumentNodeMetaData {
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

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "line-number:input-event", linenum_input_event);
    }
}

pub fn linenum_input_event(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View>>,
) {
    let v = view.read();

    // explicit focus on text view
    let mode_ctx = v.mode_ctx::<LineNumberModeContext>("line-number-mode");
    env.focus_locked_on = None;

    let evt = v.input_ctx.trigger.last();
    match evt {
        Some(InputEvent::ButtonPress(ref button_event)) => match button_event {
            ButtonEvent {
                mods:
                    KeyModifiers {
                        ctrl: _,
                        alt: _,
                        shift: _,
                    },
                x: _,
                y: _,
                button: _,
            } => {
                set_focus_on_vid(&mut editor, &mut env, mode_ctx.text_vid);
            }
        },

        Some(InputEvent::ButtonRelease(ref button_event)) => match button_event {
            ButtonEvent {
                mods:
                    KeyModifiers {
                        ctrl: _,
                        alt: _,
                        shift: _,
                    },
                x: _,
                y: _,
                button: _,
            } => {
                set_focus_on_vid(&mut editor, &mut env, mode_ctx.text_vid);
            }
        },

        Some(InputEvent::PointerMotion(PointerEvent {
            x: _,
            y: _,
            mods: _,
        })) => {}

        _ => {
            dbg_println!("LINENUM unhandled event {:?}", evt);
            return;
        }
    };
}

pub struct LineNumberModeContext {
    // add per view fields
    linenum_vid: view::Id,
    text_vid: view::Id,
}

struct LineNumberModeDocEventHandler {
    pub count: usize,
}

impl<'a> Mode for LineNumberMode {
    fn name(&self) -> &'static str {
        &"line-number-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc line-number-mode ctx");
        let ctx = LineNumberModeContext {
            linenum_vid: view::Id(0),
            text_vid: view::Id(0),
        };
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

        let meta = DOC_METADATA_MAP.write();
        let meta = meta.get(&doc_id);
        let mut meta = meta.as_ref().unwrap().write();

        if !meta.cb_installed {
            let cb = Box::new(LineNumberModeDocEventHandler { count: 0 });

            self.doc_subscription = doc.register_subscriber(cb);

            meta.cb_installed = true;
        }
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        // setup input map for core actions
        let input_map = build_input_event_map(LINENUM_INPUT_MAP).unwrap();
        let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push((self.name(), input_map));

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
        src_view: &mut View<'static>,
        parent: Option<&mut View<'static>>,
    ) {
        dbg_println!(
            "dbg LINENUM on_view_event src: {:?} dst: {:?}, event {:?} src_view {:?}",
            src,
            dst,
            event,
            src_view.id
        );

        match event {
            ViewEvent::Subscribe => {
                if src.id == dst.id {
                    // ignore self subscription
                    return;
                }

                let mut linenum_view = editor.view_map.get(&dst.id).unwrap().write();
                let mut mode_ctx =
                    linenum_view.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");

                mode_ctx.text_vid = src.id;
                mode_ctx.linenum_vid = dst.id;
            }

            ViewEvent::PreComposition => {
                if src_view.id != dst.id {
                    return;
                }

                let text_view = src_view;

                // TODO(ceg): resize line-number view
                let doc = text_view.document();
                let doc = doc.as_ref().unwrap().read();
                let max_offset = doc.size() as u64 + 1;
                let width = if !doc.indexed {
                    // '@offset '
                    1 + num_digit(max_offset) + 1
                } else {
                    let ret = get_document_byte_count_at_offset(&doc, '\n' as usize, max_offset);
                    let n = num_digit(ret.0 + 1); // nb line = line count + 1

                    // 'xxxx '
                    n + 1
                };

                if let Some(p_view) = parent {
                    p_view.children[text_view.layout_index.unwrap()].layout_op =
                        LayoutOperation::Fixed {
                            size: width as usize,
                        };
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

            DocumentEvent::NodeAdded { node_index: _ } => {}

            DocumentEvent::NodeRemoved { node_index: _ } => {}

            DocumentEvent::NodeChanged { node_index: _ } => {}

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
        let text_vid = mode_ctx.text_vid;
        let src = editor.view_map.get(&text_vid).unwrap().read();
        self.line_offsets = src.screen.read().line_offset.clone();

        self.line_number.clear();

        let doc = src.document();
        let doc = doc.as_ref().unwrap().read();

        if !doc.indexed {
            return;
        }

        // call to get_document_byte_count_at_offset are SLOW : compute only the first line
        // and read the target screen to compute relative line count
        for offset in self.line_offsets.iter().take(1) {
            let n = get_document_byte_count_at_offset(&doc, '\n' as usize, offset.0);
            self.line_number.push((offset.0, n));
        }

        if self.line_number.is_empty() {
            return;
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
                    if !cell.cpi.metadata && cell.cpi.cp == '\n' {
                        line_number += 1;
                        break;
                    }
                }
            }
            self.line_number.push((offset, (line_number as u64, None)));
        }
    }

    fn run(&mut self, _view: &View, env: &mut LayoutEnv) {
        env.screen.clear();

        let w = env.screen.width();

        // show line numbers
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

        // show offsets
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

    fn finish(&mut self, _: &View, _: &mut LayoutEnv) {}
}
