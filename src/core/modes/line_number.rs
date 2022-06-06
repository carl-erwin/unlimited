/*
    TODO(ceg): provide the goto-line,goto-offset functions

    We want:

    An Updated collection of metadata that tracks number of newlines in the buffer sub-blocks

    The root contains the total count of newlines

    NB: This is tied to the buffer implementation

                 (root)
                   |
               [ 3 + 6 ]
             /          \
        [ 1 + 2 ]     [ 2 + 4 ]
       /        \    /        \
    [1]        [2]  [2]       [4]

    each time a node is indexed
    the buffer impl must call update hierarchy with the build metadata diff

    the mode subscribes to the buffer events

    When a node is indexed/added/removed, the buffer notify us
    Then we build the node metadata
    and ask to update the hierarchy.


    To be fully async we must: have a shadowed tree that matched the buffer internal representation ?
    and keep a per node buffer_revision

    Must we re-index before remove ?
*/

use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;
use std::sync::Arc;

use crate::core::event::*;

use lazy_static::lazy_static;
use std::collections::HashMap;
use std::collections::HashSet;

use super::Mode;

use crate::core::codepointinfo::CodepointInfo;

use crate::core::editor::get_view_by_id;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_view_id;
use crate::core::editor::InputStageActionMap;
use crate::core::event::input_map::build_input_event_map;

use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::buffer;
use crate::core::buffer::get_byte_count;
use crate::core::buffer::get_byte_count_at_offset;
pub use buffer::find_nth_byte_offset;

use crate::core::buffer::Buffer;
use crate::core::buffer::BufferEvent;
use crate::core::buffer::BufferEventCb;

use crate::core::view::LayoutEnv;
use crate::core::view::ScreenOverlayFilter;

use crate::core::view;
use crate::core::view::LayoutOperation;
use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::modes::text_mode::mark::Mark;
use crate::core::modes::text_mode::TextModeContext;

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

// buffer meta data map
lazy_static! {
    static ref BUFFER_METADATA_MAP: Arc<RwLock<HashMap<buffer::Id, RwLock<LineNumberBufferMetaData>>>> =
        Arc::new(RwLock::new(HashMap::new()));

    // move to core ?
    static ref BUFFER_ID_TO_VIEW_ID_MAP: Arc<RwLock<HashMap<buffer::Id, HashSet<view::Id>>>> =
        Arc::new(RwLock::new(HashMap::new()));
}

struct LineNumberBufferMetaData {
    cb_installed: bool,
    _root_idx: Option<usize>,
}

impl LineNumberBufferMetaData {
    pub fn new() -> Self {
        dbg_println!("LineNumberBufferMetaData");
        LineNumberBufferMetaData {
            cb_installed: false,
            _root_idx: None,
        }
    }
}

struct _LineNumberBufferNodeMetaData {
    nl_count: u64,
    cr_count: u64,
    lf_count: u64,
}

impl _LineNumberBufferNodeMetaData {
    pub fn _new() -> Self {
        dbg_println!("LineNumberBufferNodeMetaData");
        _LineNumberBufferNodeMetaData {
            nl_count: 0,
            cr_count: 0,
            lf_count: 0,
        }
    }
}

pub struct LineNumberMode {
    // add common fields
    buffer_subscription: usize,
}

impl LineNumberMode {
    pub fn new() -> Self {
        dbg_println!("LineNumberMode");
        LineNumberMode {
            buffer_subscription: 0,
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
    env.focus_locked_on_view_id = None;

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
                set_focus_on_view_id(&mut editor, &mut env, mode_ctx.text_view_id);
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
                set_focus_on_view_id(&mut editor, &mut env, mode_ctx.text_view_id);
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
    linenum_view_id: view::Id,
    text_view_id: view::Id,

    pub start_line: Option<u64>,
    pub start_col: Option<u64>,
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
            linenum_view_id: view::Id(0),
            text_view_id: view::Id(0),
            start_line: None,
            start_col: None,
        };
        Box::new(ctx)
    }

    fn configure_buffer(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        buffer: &mut Buffer<'static>,
    ) {
        // allocate buffer meta data
        let buffer_id = buffer.id;

        BUFFER_METADATA_MAP
            .as_ref()
            .write()
            .entry(buffer_id)
            .or_insert(RwLock::new(LineNumberBufferMetaData::new()));

        let meta = BUFFER_METADATA_MAP.write();
        let meta = meta.get(&buffer_id);
        let mut meta = meta.as_ref().unwrap().write();

        if !meta.cb_installed {
            let cb = Box::new(LineNumberModeDocEventHandler { count: 0 });

            self.buffer_subscription = buffer.register_subscriber(cb);

            meta.cb_installed = true;
        }
    }

    fn on_buffer_event(
        &self,
        editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        _event: &BufferEvent,
        view: &mut View<'static>,
    ) {
        dbg_println!("mode '{}' on_buffer_event: event {:?}", self.name(), _event);

        if let Some(buffer) = view.buffer() {
            let max_offset = buffer.read().size() as u64;

            let position = buffer.read().start_position;
            if let Some(target_line) = position.line {
                dbg_println!("goto line {:?} ?", target_line);

                let offset = if target_line <= 1 {
                    0
                } else {
                    let line_number = target_line.saturating_sub(1);
                    if let Some(offset) =
                        find_nth_byte_offset(&buffer.read(), '\n' as u8, line_number)
                    {
                        offset + 1
                    } else {
                        max_offset as u64
                    }
                };

                dbg_println!("goto line {:?} offset : {}", target_line, offset);

                let lnm = view.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");
                let text_view_id = lnm.text_view_id;

                let text_view = get_view_by_id(editor, text_view_id);
                let mut text_view = text_view.write();

                // check offscreen
                text_view.start_offset = offset;

                // update marks ?
                let tm = text_view.mode_ctx_mut::<TextModeContext>("text-mode");

                tm.marks.clear();
                tm.marks.push(Mark { offset });
            }
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

        let buffer_id = view.buffer().unwrap().read().id;
        let view_id = view.id;

        // move to core ?
        BUFFER_ID_TO_VIEW_ID_MAP
            .as_ref()
            .write()
            .entry(buffer_id)
            .or_insert_with(HashSet::new)
            .insert(view_id);
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

                let linenum_view = get_view_by_id(editor, dst.id);
                let mut linenum_view = linenum_view.write();

                let mut mode_ctx =
                    linenum_view.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");

                mode_ctx.text_view_id = src.id;
                mode_ctx.linenum_view_id = dst.id;
            }

            ViewEvent::PreLayoutSizing => {
                if src_view.id == dst.id {
                    return;
                }

                let text_view = src_view;
                let linenum_view = get_view_by_id(editor, dst.id);
                let linenum_view = linenum_view.read();

                // TODO(ceg): resize line-number view
                let buffer = text_view.buffer();
                let buffer = buffer.as_ref().unwrap().read();
                let max_offset = buffer.size() as u64 + 1;
                let width = if !buffer.indexed {
                    // '@offset '
                    1 + num_digit(max_offset) + 1
                } else {
                    let ret = get_byte_count(&buffer, '\n' as usize).unwrap_or(0);
                    let n = num_digit(ret + 1); // nb line = line count + 1

                    // 'xxxx '
                    n + 1
                };

                if let Some(p_view) = parent {
                    p_view.children[linenum_view.layout_index.unwrap()].layout_op =
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

impl BufferEventCb for LineNumberModeDocEventHandler {
    fn cb(&mut self, buffer: &Buffer, event: &BufferEvent) {
        self.count += 1;

        dbg_println!(
            "LineNumberModeDocEventHandler ev {:?} CB count = {}",
            event,
            self.count
        );

        match event {
            BufferEvent::BufferNodeIndexed {
                buffer_id: _,
                node_index,
            } => {
                dbg_println!(
                    "TODO index node {} with target codec  {:?}",
                    node_index,
                    event
                );
            }

            BufferEvent::BufferNodeAdded {
                buffer_id: _,
                node_index: _,
            } => {}

            BufferEvent::BufferNodeRemoved {
                buffer_id: _,
                node_index: _,
            } => {}

            BufferEvent::BufferNodeChanged {
                buffer_id: _,
                node_index: _,
            } => {}

            BufferEvent::BufferFullyIndexed { buffer_id: _ } => {}

            _ => {
                dbg_println!("unhandled event {:?}", event);
            }
        }

        buffer.show_root_node_bytes_stats();
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
        editor: &Editor<'static>,
        _env: &mut LayoutEnv,
        view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        let view = view.read();
        let mode_ctx = view.mode_ctx::<LineNumberModeContext>("line-number-mode");
        let text_view_id = mode_ctx.text_view_id;
        let src = get_view_by_id(editor, text_view_id);
        let src = src.read();
        self.line_offsets = src.screen.read().line_offset.clone();

        self.line_number.clear();

        let buffer = src.buffer();
        let buffer = buffer.as_ref().unwrap().read();

        if !buffer.indexed {
            return;
        }

        // call to get_byte_count_at_offset are SLOW : compute only the first line
        // and read the target screen to compute relative line count
        for offset in self.line_offsets.iter().take(1) {
            let n = get_byte_count_at_offset(&buffer, '\n' as usize, offset.0);
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
