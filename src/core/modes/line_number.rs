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
use crate::core::view::Id;
use crate::core::view::LayoutSize;
use crate::core::view::View;
use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::modes::text_mode::mark::Mark;
use crate::core::modes::text_mode::TextModeContext;

use crate::core::editor::config_var_get;

use crate::core::event::InputEvent;

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
    let display_mode = {
        let v = view.read();

        // explicit focus on text view
        let mode_ctx = v.mode_ctx::<LineNumberModeContext>("line-number-mode");
        let mut display_mode = mode_ctx.display_mode;
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
                    button,
                } => {
                    if *button == 0 {
                        // TODO(ceg): move mark to selected line and start selection
                    }

                    if *button == 1 {
                        // set mode
                        display_mode += 1;
                        display_mode %= 4;

                        set_focus_on_view_id(&mut editor, &mut env, mode_ctx.text_view_id);
                    }
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
                    y,
                    button,
                } => {
                    if *button == 1 {
                        // ignore right button release
                        return;
                    }

                    set_focus_on_view_id(&mut editor, &mut env, mode_ctx.text_view_id);
                    // TODO: move mark to selected line and update selection

                    // select line
                    let lnm = v.mode_ctx::<LineNumberModeContext>("line-number-mode");
                    let text_view_id = lnm.text_view_id;

                    let text_view = get_view_by_id(editor, text_view_id);
                    let mut text_view = text_view.write();

                    // check offscreen
                    let idx = *y as usize;
                    let offset = {
                        let screen = text_view.screen.read();
                        if let Some(l) = screen.get_used_line(idx) {
                            l[0].cpi.offset
                        } else {
                            None
                        }
                    };
                    if let Some(offset) = offset {
                        // update marks
                        let tm = text_view.mode_ctx_mut::<TextModeContext>("text-mode");
                        tm.marks.clear();
                        tm.marks.push(Mark::new(offset));

                        // TODO(ceg): ignore if view was change by user
                        // let msg = Message::new(0, 0, 0, Event::RefreshView);
                        // crate::core::event::pending_input_event_inc(1);
                        // editor.core_tx.send(msg).unwrap_or(());
                    }
                }
            },

            Some(InputEvent::PointerMotion(PointerEvent {
                x: _,
                y: _,
                mods: _,
            })) => { /* TODO(ceg): update selection */ }

            _ => {
                dbg_println!("LINENUM unhandled event {:?}", evt);
                return;
            }
        };

        display_mode
    };

    // save
    let mut v = view.write();
    let mode_ctx = v.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");
    mode_ctx.display_mode = display_mode;
}

pub struct LineNumberModeContext {
    // add per view fields
    linenum_view_id: view::Id,
    text_view_id: view::Id,
    display_mode: usize, // 0: absolute, 1:relative, 2:hybrid 4:offset

    pub start_line: Option<u64>,
    pub start_col: Option<u64>,
}

struct LineNumberModeBufferEventHandler {
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

    fn alloc_ctx(&self, _editor: &Editor<'static>) -> Box<dyn Any> {
        dbg_println!("alloc line-number-mode ctx");

        let ctx = LineNumberModeContext {
            linenum_view_id: view::Id(0),
            text_view_id: view::Id(0),
            display_mode: 0,
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
            let cb = Box::new(LineNumberModeBufferEventHandler { count: 0 });

            self.buffer_subscription = buffer.register_subscriber(cb);

            meta.cb_installed = true;
        }
    }

    fn configure_view(
        &mut self,
        editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        {
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
        {
            // config var

            let mode_ctx = view.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");

            let v = if let Some(display_mode) = config_var_get(&editor, "line-number-mode:display")
            {
                display_mode.trim_end().parse::<usize>().unwrap_or(0)
            } else {
                0
            };

            mode_ctx.display_mode = std::cmp::min(v, 3);
        }
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

                if src.id == Id(0) {
                    return;
                }

                if dst.id == Id(0) {
                    return;
                }

                let linenum_view = get_view_by_id(editor, dst.id);
                let mut linenum_view = linenum_view.write();

                dbg_println!("setup line_number ctx for view {:?}", dst.id);

                let mode_ctx =
                    linenum_view.mode_ctx_mut::<LineNumberModeContext>("line-number-mode");

                mode_ctx.text_view_id = src.id;
                mode_ctx.linenum_view_id = dst.id;
            }

            ViewEvent::PreLayoutSizing => {
                if src_view.id == dst.id {
                    return;
                }

                let display_mode = {
                    let linenum_view = get_view_by_id(editor, dst.id);
                    let linenum_view = linenum_view.read();

                    let mode_ctx =
                        linenum_view.mode_ctx::<LineNumberModeContext>("line-number-mode");
                    mode_ctx.display_mode
                };

                let text_view = src_view;
                let linenum_view = get_view_by_id(editor, dst.id);
                let linenum_view = linenum_view.read();

                // TODO(ceg): resize line-number view
                let buffer = text_view.buffer();
                let buffer = buffer.as_ref().unwrap().read();
                let max_offset = buffer.size() as u64 + 1;
                let width = if !buffer.indexed || display_mode == 3 {
                    // '@offset '
                    1 + num_digit(max_offset)
                } else {
                    let ret = get_byte_count(&buffer, '\n' as usize).unwrap_or(0);
                    let n = num_digit(ret + 1); // nb line = line count + 1

                    // 'xxxx '
                    n
                };

                let width = match std::env::var("SINGLE_VIEW") {
                    Ok(_) => 0,
                    _ => width,
                };

                if let Some(p_view) = parent {
                    p_view.children[linenum_view.layout_index.unwrap()].layout_op =
                        LayoutSize::Fixed {
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

impl BufferEventCb for LineNumberModeBufferEventHandler {
    fn cb(&mut self, buffer: &Buffer, event: &BufferEvent) {
        self.count += 1;

        dbg_println!(
            "LineNumberModeBufferEventHandler ev {:?} CB count = {}",
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
        }

        buffer.show_root_node_bytes_stats();
    }
}

///////////////////////////////////////////////////////////////////////////////////////////////////

#[derive(Debug)]
pub struct LineNumberOverlayFilter {
    mark_offset: u64,
    mark_line: u64,
    line_offsets: Vec<(u64, u64)>,
    line_number: Vec<(u64, u64, (u64, Option<usize>))>, // (offset, end_offset, (line_num, node_index))
}

impl LineNumberOverlayFilter {
    pub fn new() -> Self {
        LineNumberOverlayFilter {
            mark_offset: 0,
            mark_line: 0,
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

        if text_view_id == Id(0) {
            return;
        }

        let src = get_view_by_id(editor, text_view_id);
        let src = src.read();
        self.line_offsets = src.screen.read().line_offset.clone();

        // get main mark offset
        let tm = src.mode_ctx::<TextModeContext>("text-mode");
        self.mark_offset = tm.marks[tm.mark_index].offset;

        let buffer = src.buffer();
        let buffer = buffer.as_ref().unwrap().read();

        if !buffer.indexed {
            return;
        }

        self.line_number.clear();
        let screen = src.screen.as_ref().read();

        if self.line_offsets.is_empty() {
            return;
        }

        let mut prev_end_cpi: Option<CodepointInfo> = None;
        let mut line_number = 0;
        for i in 0..screen.line_index.len() {
            let mut offset = 0;
            let mut end_offset = 0;

            if let Some(l) = screen.get_used_line(i) {
                if let Some(cell) = l.first() {
                    offset = cell.cpi.offset.unwrap();
                }

                line_number = {
                    // avoid slow call to get_byte_count_at_offset
                    if let Some(prev_cpi) = prev_end_cpi {
                        let n = if prev_cpi.cp == '\n' && prev_cpi.metadata == false {
                            1
                        } else {
                            0
                        };
                        line_number + n
                    } else {
                        let offset = &self.line_offsets[i];
                        let n = get_byte_count_at_offset(&buffer, '\n' as usize, offset.0);
                        1 + n.0
                    }
                };

                if let Some(cell) = l.last() {
                    prev_end_cpi = Some(cell.cpi.clone());
                    end_offset = cell.cpi.offset.unwrap();
                }

                end_offset = std::cmp::max(offset, end_offset);

                if offset <= self.mark_offset && self.mark_offset <= end_offset {
                    self.mark_line = line_number;
                }

                let v = (offset, end_offset, (line_number as u64, None));
                self.line_number.push(v);
            }
        }
    }

    fn run(&mut self, view: &View, env: &mut LayoutEnv) {
        env.screen.clear();

        let w = env.screen.width();

        // TODO(ceg): move to init
        let mut color = CodepointInfo::new().style.color;
        color.0 = color.0.saturating_sub(70);
        color.1 = color.1.saturating_sub(70);
        color.2 = color.2.saturating_sub(70);

        let mut has_mark_color = CodepointInfo::new().style.color;
        has_mark_color.0 = has_mark_color.0.saturating_sub(40);
        has_mark_color.1 = has_mark_color.1.saturating_sub(40);
        has_mark_color.2 = has_mark_color.2.saturating_sub(40);

        let mode_ctx = view.mode_ctx::<LineNumberModeContext>("line-number-mode");
        let display_mode = mode_ctx.display_mode;

        // show line numbers
        if !self.line_number.is_empty() && display_mode != 3 {
            let mut prev_line = 0;
            for (idx, e) in self.line_number.iter().enumerate() {
                let cur_line_num = e.2 .0;

                if idx > 0 && cur_line_num == prev_line {
                    // skip wrapped line
                    env.screen.select_next_line_index();
                    continue;
                }

                let mut enable_padding = true;

                // show relative lines (add keyboard toggle)
                let s = if display_mode > 0 {
                    if self.mark_line > cur_line_num {
                        format!("{}", self.mark_line - cur_line_num)
                    } else if self.mark_line < cur_line_num {
                        format!("{}", cur_line_num - self.mark_line)
                    } else {
                        enable_padding = false;
                        if display_mode == 1 {
                            format!("0")
                        } else {
                            format!("{}", self.mark_line)
                        }
                    }
                } else {
                    // absolute
                    format!("{}", cur_line_num)
                };

                prev_line = cur_line_num;

                if enable_padding {
                    let padding = w.saturating_sub(s.len());
                    // left-pad
                    for _ in 0..padding {
                        env.screen.push(&CodepointInfo::new());
                    }
                }

                let has_mark = self.mark_line == cur_line_num;

                let final_color = if has_mark { has_mark_color } else { color };

                let cur_line_idx = env.screen.current_line_index();
                for c in s.chars() {
                    let mut cpi = CodepointInfo::new();
                    cpi.displayed_cp = c;
                    cpi.style.color = final_color;
                    cpi.style.is_bold = has_mark;
                    env.screen.push(&cpi);
                }
                if cur_line_idx == env.screen.current_line_index() {
                    // NB screen.push selects next line automatically
                    env.screen.select_next_line_index();
                }
            }
            return;
        }

        // show offsets
        for e in self.line_offsets.iter() {
            let s = format!("@{}", e.0);
            let has_mark = self.mark_offset >= e.0 && self.mark_offset <= e.1;
            let final_color = if has_mark { has_mark_color } else { color };

            let cur_line_idx = env.screen.current_line_index();
            for c in s.chars() {
                let mut cpi = CodepointInfo::new();
                cpi.displayed_cp = c;
                cpi.style.color = final_color;
                cpi.style.is_bold = has_mark;
                env.screen.push(&cpi);
            }
            if cur_line_idx == env.screen.current_line_index() {
                // NB screen.push selects next line automatically
                env.screen.select_next_line_index();
            }
        }
    }

    fn finish(&mut self, _: &View, _: &mut LayoutEnv) {}
}
