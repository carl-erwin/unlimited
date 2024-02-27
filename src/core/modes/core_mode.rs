use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;
use std::sync::Arc;

use super::Mode;

use crate::core::buffer::BufferBuilder;
use crate::core::buffer::BufferKind;

use crate::core::editor::check_view_by_id;
use crate::core::editor::get_view_by_id;
use crate::core::editor::remove_view_by_id;

use crate::core::editor::register_input_stage_action;

use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;

use crate::core::view;
use crate::core::view::ChildView;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutSize;
use crate::core::view::View;

use crate::core::build_view_layout_from_json_str;

static CORE_INPUT_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "F2" } ],                                               "action": "select-previous-view" },
     { "in": [{ "key": "F3" } ],                                               "action": "select-next-view" },
     { "in": [{ "key": "F4"     }],                                            "action": "toggle-debug-print" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+s" } ],                      "action": "save-buffer" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+q" } ],                      "action": "application:quit" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+x" }, { "key": "ctrl+q" } ], "action": "application:quit-abort" },
     { "in": [{ "key": "F1" } ],                                               "action": "help-pop-up" }
    ]
  }
]"#;

static CORE_QUIT_ABORT_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "y" } ],    "action": "application:quit-abort-yes" },
     { "in": [{ "key": "n" } ],    "action": "application:quit-abort-no" },
     { "default": [],              "action": "application:quit-abort-no" }
   ]
  }

]"#;

impl<'a> Mode for CoreMode {
    fn name(&self) -> &'static str {
        &"core-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc core-mode ctx");
        let ctx = CoreModeContext {};
        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        // setup input map for core actions
        let input_map = build_input_event_map(CORE_INPUT_MAP).unwrap();
        let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push((self.name(), input_map));
    }
}

pub struct CoreMode {
    // add common fields
}
pub struct CoreModeContext {
    // add common fields
}

impl CoreMode {
    pub fn new() -> Self {
        dbg_println!("CoreMode");
        CoreMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "toggle-debug-print", toggle_dgb_print);

        register_input_stage_action(&mut map, "application:quit", application_quit);
        register_input_stage_action(
            &mut map,
            "application:quit-abort",
            application_quit_abort_yes,
        );
        register_input_stage_action(
            &mut map,
            "application:quit-abort-yes",
            application_quit_abort_yes,
        );
        register_input_stage_action(
            &mut map,
            "application:quit-abort-no",
            application_quit_abort_no,
        );

        register_input_stage_action(&mut map, "help-pop-up", help_popup);

        register_input_stage_action(&mut map, "save-buffer", save_buffer); // core ?
        register_input_stage_action(&mut map, "split-vertically", split_vertically);
        register_input_stage_action(&mut map, "split-horizontally", split_horizontally);
        register_input_stage_action(&mut map, "destroy-view", destroy_view);

        register_input_stage_action(&mut map, "increase-left", increase_left);
        register_input_stage_action(&mut map, "decrease-left", decrease_left);
        register_input_stage_action(&mut map, "increase-right", increase_right);
        register_input_stage_action(&mut map, "decrease-right", decrease_right);

        register_input_stage_action(&mut map, "increase-right", increase_right);
        register_input_stage_action(&mut map, "decrease-right", decrease_right);

        register_input_stage_action(&mut map, "select-next-view", select_next_view);
        register_input_stage_action(&mut map, "select-previous-view", select_previous_view);
    }
}

// Mode "core"
pub fn select_next_view(editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    env.root_view_index = std::cmp::min(env.root_view_index + 1, editor.root_views.len() - 1);
    env.root_view_id = editor.root_views[env.root_view_index];
    dbg_println!("select {:?}", env.root_view_id);
}

pub fn select_previous_view(editor: &mut Editor, env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    env.root_view_index = env.root_view_index.saturating_sub(1);
    env.root_view_id = editor.root_views[env.root_view_index];
    dbg_println!("select {:?}", env.root_view_id);
}

pub fn application_quit(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // TODO(ceg): change this
    // editor.changed_buffer : HashSet<buffer::Id>
    // if editor.change_buffers.len() != 0

    let buffer = { view.read().buffer().unwrap() };
    let buffer = buffer.read();
    if !buffer.changed {
        env.quit = true;
    } else {
        application_quit_abort_setup(&mut editor, &mut env, &view);
    }
}

pub fn application_quit_abort_setup(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let status_view_id = view::get_status_view(editor, env, view);

    dbg_println!("DOC CHANGED !\n");
    dbg_println!("STATUS VID = {:?}", status_view_id);

    if let Some(svid) = status_view_id {
        let status_view = get_view_by_id(editor, svid);
        //
        let buffer = status_view.read().buffer().unwrap();
        let mut buffer = buffer.write();
        // clear doc
        let sz = buffer.size();
        buffer.remove(0, sz, None);
        // set status text
        let text = "Modified buffers exist. Really quit? y/n";
        let bytes = text.as_bytes();
        buffer.insert(0, bytes.len(), &bytes);

        // push new input map for y/n
        {
            let mut v = view.write();
            // lock focus on v
            // env.focus_locked_on_view_id = Some(v.id);

            dbg_println!("configure quit-abort  {:?}", v.id);
            v.input_ctx.stack_pos = None;
            let input_map = build_input_event_map(CORE_QUIT_ABORT_MAP).unwrap();
            let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(("core-mode", input_map));
            // TODO(ceg): add lock flag
            // to not exec lower input level
        }
    } else {
        // TODO(ceg): log missing status mode
    }
}

pub fn application_quit_abort_yes(
    _editor: &mut Editor,
    env: &mut EditorEnv,
    _view: &Rc<RwLock<View>>,
) {
    env.quit = true;
}

pub fn application_quit_abort_no(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let v = view.write();
        let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.pop();
        // unlock focus
        // env.focus_locked_on_view_id = None;
    }

    // reset status view : TODO(ceg): view::reset_status_view(&editor, view);
    let status_view_id = view::get_status_view(editor, env, view);
    if let Some(status_view_id) = status_view_id {
        let status_view = get_view_by_id(editor, status_view_id);
        let buffer = status_view.read().buffer().unwrap();
        let mut buffer = buffer.write();
        // clear buffer
        let sz = buffer.size();
        buffer.remove(0, sz, None);
    }
}

pub fn toggle_dgb_print(_editor: &mut Editor, _env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    crate::core::toggle_dbg_println();
}

pub fn save_buffer(editor: &mut Editor<'static>, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = view.write();

    let buffer_id = {
        let buffer = v.buffer().unwrap();
        {
            // - needed ? already syncing ? -
            let buffer = buffer.read();
            if !buffer.changed || buffer.is_syncing {
                // TODO(ceg): ensure all other places are checking this flag, all buffer....write()
                // better, some permissions mechanism ?
                // buffer.access_permissions = r-
                // buffer.access_permissions = -w
                // buffer.access_permissions = rw
                return;
            }
        }

        // - set sync flag -
        {
            let mut buffer = buffer.write();
            let buffer_id = buffer.id;
            buffer.is_syncing = true;
            buffer_id
        }
    };

    // - send sync job to worker -
    //
    // NB: We must take the buffer clone from Editor not View
    // because lifetime(editor) >= lifetime(view)
    // ( view.doc is a clone from editor.buffer_map ),
    // doing this let us avoid the use manual lifetime annotations ('static)
    // and errors like "data from `view` flows into `editor`"
    let buffer_map = editor.buffer_map.clone();
    let buffer_map = buffer_map.read();

    let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

    if let Some(buffer) = buffer_map.get(&buffer_id) {
        let msg = Message {
            seq: 0,
            input_ts: 0,
            ts,
            event: Event::SyncTask {
                buffer: Arc::clone(buffer),
            },
        };
        editor.worker_tx.send(msg).unwrap_or(());
    }
}

pub fn layout_view_ids_with_direction(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    parent_id: view::Id,
    width: usize,
    height: usize,
    dir: view::LayoutDirection,
    layout_ops: &Vec<LayoutSize>,
    view_ids: &Vec<view::Id>,
) {
    let parent = get_view_by_id(editor, parent_id);
    let mut parent = parent.write();

    parent.layout_direction = dir;
    let sizes = if dir == LayoutDirection::Vertical {
        view::compute_layout_sizes(height, &layout_ops) // options ? for ret size == 0
    } else {
        view::compute_layout_sizes(width, &layout_ops) // options ? for ret size == 0
    };

    dbg_println!(
        "LAYOUT WITH DIRECTION {:?} = SIZE {:?} NB OPS {}",
        dir,
        sizes,
        layout_ops.len()
    );

    let mut x = parent.x;
    let mut y = parent.y;

    for (idx, size) in sizes.iter().enumerate() {
        let size = std::cmp::max(1, *size); // screen require 1x1 as min
        let (width, height) = match dir {
            LayoutDirection::Vertical => (width, size),
            LayoutDirection::Horizontal => (size, height),
            _ => {
                return;
            }
        };

        let view = get_view_by_id(editor, view_ids[idx]);
        let mut view = view.write();

        view.x = x;
        view.y = y;
        view.width = width;
        view.height = height;
        view.parent_id = Some(parent.id);
        view.start_offset = parent.start_offset;
        view.layout_index = Some(idx);

        // TODO(ceg): move this to caller
        parent.children.push(ChildView {
            id: view.id,
            layout_op: layout_ops[idx].clone(),
        });

        match dir {
            LayoutDirection::Vertical => {
                x += size;
            }
            LayoutDirection::Horizontal => {
                y += size;
            }
            _ => {
                return;
            }
        }
    }
}

fn find_first_splittable_parent(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> Option<view::Id> {
    let mut start_id = { view.read().id };

    loop {
        let view = get_view_by_id(editor, start_id);
        let v = view.read();
        if v.is_splittable {
            return Some(v.id);
        }
        start_id = v.parent_id?;
    }
}

/*
  To split a given View (view_to_split)

  - we create a new View (new_parent)
  - we create a new Splitter (splitter)
  - we create a clone of view_to_split  (view_clone)
  - replace view_to_split by new_parent
  - put view_to_split as child of new_parent
  - put splitter as child of new_parent
  - put view_clone as child of new_parent

    [ parent ]
       |
    [ view_to_split ]

  will become

    [ parent ]
       |
    [ new_parent ]
       |
   --------------------------------
   |                 |            |
   |                 |            |
   |                 |            |
   [ view_to_split ] [ splitter ] [ view_clone ]


    look for first view with the is_splittable flag set

    create a new_parent with width height

    create a new_view with same params/modes

    split_with_direction -> layout_view_ids_with_directions()

    layout_view_ids_with_direction(&mut editor,
        &mut env,
        &mut v,
        width,
        height,
        LayoutDirection::Horizontal,
        Vec<view::Id>);

*/
struct SplitInfo {
    view_to_split_id: view::Id,
    parent_id: Option<view::Id>,
    x: usize,
    y: usize,
    width: usize,
    height: usize,
    layout_index: Option<usize>,
}

fn build_split_info(view: &Rc<RwLock<View<'static>>>, dir: view::LayoutDirection) -> SplitInfo {
    let v = view.read();

    dbg_println!("SPLITTING {:?}  {:?}", dir, v.id);

    let (width, height) = {
        let screen = v.screen.read();
        (screen.width(), screen.height())
    };

    // compute left and right size as current View / 2
    // get screen

    SplitInfo {
        view_to_split_id: v.id,
        parent_id: v.parent_id,
        x: v.x,
        y: v.y,
        width,
        height,
        layout_index: v.layout_index,
    }
}

/*
    TODO(ceg): build split-vertical-layout.json

        1 - clone view, set clone offset to view offset
        2 - create split-group layout (empty_view(1) + splitter + empty_view(2) )
        3 - replace view by split-group (in view's parent)
        4 - move view  to left  leaf empty_view(1)
        5 - move clone to right leaf empty_view(2)

        replace view's parent index with split group
*/
pub fn split_view_with_direction(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    dir: view::LayoutDirection,
) -> Option<()> {
    dbg_println!("try splitting id {:?}", view.read().id);

    let id = find_first_splittable_parent(editor, env, view)?; // group leader (ex: simple_view)

    let pview = get_view_by_id(editor, id);

    let json_attr = if let Some(ref json_attr) = pview.read().json_attr {
        json_attr.clone()
    } else {
        dbg_println!("cannot clone id {:?}, no json attr found", view.read().id);
        return None;
    };

    let buffer = pview.read().buffer();

    // create view clone
    let view_clone_id = build_view_layout_from_json_str(editor, env, buffer, &json_attr, 0)?;

    dbg_println!("json attr {:?}", json_attr);

    let split_info = build_split_info(&pview, dir); // remove

    // create new parent (will replace [view_to_split] in the hierarchy)
    let mut new_parent = View::new(
        &mut editor,
        &mut env,
        split_info.parent_id,         // group leader's parent
        (split_info.x, split_info.y), // relative to parent, i32 allow negative moves?
        (split_info.width, split_info.height),
        None,
        &vec![], // TODO(ceg): add core mode for save/quit/quit/abort/split{V,H}
        0,
        LayoutDirection::NotSet,
        LayoutSize::Percent { p: 100.0 },
    );

    new_parent.is_group = true;

    // add some restrictions
    const WIDTH_MIN: usize = 16;
    const HEIGHT_MIN: usize = 16;
    match dir {
        view::LayoutDirection::Horizontal => {
            if split_info.width <= WIDTH_MIN {
                dbg_println!(
                    "view to split not wide enough : width {} <= {}",
                    split_info.width,
                    WIDTH_MIN
                );
                return None;
            }
        }
        view::LayoutDirection::Vertical => {
            if split_info.height <= HEIGHT_MIN {
                dbg_println!(
                    "view to split not wide enough : height {} <= {}",
                    split_info.height,
                    HEIGHT_MIN
                );
                return None;
            }
        }

        _ => {
            return None;
        }
    }

    // children_layout_and_modes
    let layout_ops = vec![
        LayoutSize::Percent { p: 50.0 },        // left (view_to_split)
        LayoutSize::Fixed { size: 1 },          // splitter
        LayoutSize::RemainPercent { p: 100.0 }, // right (view_clone)
    ];

    // new parent replaces view_to_split i parent(view_to_split)
    new_parent.layout_index = split_info.layout_index;
    let new_parent_id = new_parent.id;
    // insert new_parent into editor global map
    editor.add_view(new_parent_id, new_parent);

    dbg_println!("new parent = {:?}", new_parent_id);

    // update grand parent, replace v1_id by p2_id
    if let Some(parent_id) = split_info.parent_id {
        if let Some(gp) = check_view_by_id(editor, parent_id) {
            let mut gp = gp.write();
            if let Some(layout_index) = split_info.layout_index {
                gp.children[layout_index].id = new_parent_id;
            }
        }
    }

    // create splitter
    let splitter_id = {
        let splitter_mode = match dir {
            view::LayoutDirection::Horizontal => vec!["vsplit-mode".to_owned()],
            view::LayoutDirection::Vertical => vec!["hsplit-mode".to_owned()],
            _ => panic!(),
        };

        let splitter = View::new(
            &mut editor,
            &mut env,
            Some(new_parent_id),
            (split_info.x, split_info.y), // relative to parent, i32 allow negative moves?
            (split_info.width, split_info.height),
            None,
            &splitter_mode,
            0,
            LayoutDirection::NotSet,
            LayoutSize::Percent { p: 100.0 },
        );

        let splitter_id = splitter.id;
        editor.add_view(splitter_id, splitter);

        dbg_println!("splitter_id = {:?}", splitter_id);

        splitter_id
    };

    // set view__to_split | splitter | view_clone  children of new_parent
    let view_ids = vec![split_info.view_to_split_id, splitter_id, view_clone_id];
    layout_view_ids_with_direction(
        &mut editor,
        &mut env,
        new_parent_id,
        split_info.width,
        split_info.height,
        dir,
        &layout_ops,
        &view_ids,
    );

    Some(())
}

pub fn split_vertically(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    split_view_with_direction(
        &mut editor,
        &mut env,
        view,
        view::LayoutDirection::Horizontal,
    );
}

pub fn split_horizontally(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    split_view_with_direction(&mut editor, &mut env, view, view::LayoutDirection::Vertical);
}

// quick hack ignoring other children
pub fn increase_layout_op(
    op: &LayoutSize,
    max_size: usize,
    cur_size: usize,
    diff: usize,
) -> LayoutSize {
    dbg_println!(
        "INC LAYOUT OP {:?}, max_size = {} max_size, cur_size {} diff {}",
        op,
        max_size,
        cur_size,
        diff
    );

    let new_op = match *op {
        LayoutSize::Fixed { size } if size < max_size => LayoutSize::Fixed { size: size + 1 },
        LayoutSize::Percent { p } => {
            if cur_size + diff >= max_size {
                return op.clone();
            }
            let expect_p = ((cur_size + diff) as f32 * p) / cur_size as f32;
            dbg_println!("LAYOUT expect_p = {}", expect_p);
            LayoutSize::Percent { p: expect_p }
        }
        LayoutSize::RemainPercent { p } if p < 99.0 => {
            let unit = max_size as f32 / 100.0;
            LayoutSize::RemainPercent { p: p + unit }
        }
        LayoutSize::RemainMinus { minus } => {
            dbg_println!(
                "LAYOUT = max_size{} - minus*100{} / 100 = {}",
                minus * 100,
                max_size,
                max_size.saturating_sub(minus * 100) / 100
            );
            LayoutSize::RemainMinus {
                minus: ((minus * 100 + max_size) / 100) - 1,
            }
        }
        _ => op.clone(),
    };

    dbg_println!("INC LAYOUT NEW OP {:?}", new_op);

    new_op
}

// quick hack ignoring other children
pub fn decrease_layout_op(
    op: &LayoutSize,
    // TODO(ceg): min_size: usize,
    max_size: usize,
    cur_size: usize,
    diff: usize, // decrease amount
) -> LayoutSize {
    dbg_println!(
        "DEC LAYOUT OP {:?}, max_size = {} max_size, cur_size {} diff {}",
        op,
        max_size,
        cur_size,
        diff
    );

    let new_op = match *op {
        LayoutSize::Fixed { size } if size > diff => LayoutSize::Fixed { size: size - diff },
        LayoutSize::Percent { p } => {
            if cur_size <= diff {
                return op.clone();
            }

            let expect_p = ((cur_size - diff) as f32 * p) / cur_size as f32;
            dbg_println!("LAYOUT expect_p = {}", expect_p);
            LayoutSize::Percent { p: expect_p }
        }

        LayoutSize::RemainPercent { p } if p > 2.0 => {
            let unit = (max_size as f32) / 100.0;
            LayoutSize::RemainPercent { p: p - unit }
        }
        LayoutSize::RemainMinus { minus } => {
            dbg_println!(
                "LAYOUT = minus * 100 {} + max_size {} / 100 = {}",
                minus * 100,
                max_size,
                (minus * 100 + max_size) / 100
            );
            if ((minus * 100 + max_size) / 100) + 1 > 100 {
                return op.clone();
            }
            LayoutSize::RemainMinus {
                minus: ((minus * 100 + max_size) / 100) + 1,
            }
        }
        _ => op.clone(),
    };

    dbg_println!("DEC LAYOUT NEW OP {:?}", new_op);

    new_op
}

pub fn increase_left(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.write();
    if v.parent_id.is_none() {
        return;
    }

    let pvid = v.parent_id.unwrap();
    let pv = get_view_by_id(editor, pvid);
    let mut pv = pv.write();

    let lidx = v.layout_index.unwrap();
    dbg_println!("lidx = {}", lidx);
    if lidx < 2 {
        return;
    }
    let lidx = lidx - 2; // take left sibling

    let max_size = pv.screen.read().width();
    let new_op = decrease_layout_op(&pv.children[lidx].layout_op, max_size, max_size, 1);
    pv.children[lidx].layout_op = new_op;
}

pub fn decrease_left(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.write();
    if v.parent_id.is_none() {
        return;
    }

    let pvid = v.parent_id.unwrap();
    let pv = get_view_by_id(editor, pvid);
    let mut pv = pv.write();

    let lidx = v.layout_index.unwrap();
    dbg_println!("lidx = {}", lidx);
    if lidx < 2 {
        return;
    }
    let lidx = lidx - 2; // take previous sibling

    let max_size = pv.screen.read().width();
    let cur_size = v.screen.read().width();
    let new_op = increase_layout_op(&pv.children[lidx].layout_op, max_size, cur_size, 1);
    pv.children[lidx].layout_op = new_op;
}

pub fn increase_right(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.write();
    if v.parent_id.is_none() {
        return;
    }

    let pvid = v.parent_id.unwrap();
    let pv = get_view_by_id(editor, pvid);
    let mut pv = pv.write();

    let lidx = v.layout_index.unwrap();
    dbg_println!("lidx = {}", lidx);
    if lidx != 0 {
        return;
    }

    let max_size = pv.screen.read().width();
    let cur_size = v.screen.read().width();
    let new_op = increase_layout_op(&pv.children[lidx].layout_op, max_size, cur_size, 1);
    pv.children[lidx].layout_op = new_op;
}

pub fn decrease_right(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.write();
    if v.parent_id.is_none() {
        return;
    }

    let pvid = v.parent_id.unwrap();
    let pv = get_view_by_id(editor, pvid);
    let mut pv = pv.write();

    let lidx = v.layout_index.unwrap();
    dbg_println!("lidx = {}", lidx);
    if lidx != 0 {
        return;
    }

    let max_size = pv.screen.read().width();
    let cur_size = v.screen.read().width();
    let new_op = decrease_layout_op(&pv.children[lidx].layout_op, max_size, cur_size, 1);
    pv.children[lidx].layout_op = new_op;
}

/*
                     Option<gparent>
                        |
                     Option<parent>
                        |
             __________|_____________________
            |                  |             \
         simple_view (1)    splitter     simple_view (2)
           /  |    \                          /  |      \
         /   |      \                       /    |       \
      /     |        \                   /       |        \
    lines text_view  vscrollbar         lines text_view  vscrollbar


                     Option<gparent>
                        |
                        |
                 simple_view (1 or 2)
                    /  |    \
                  /   |      \
               /     |        \
             lines text_view  vscrollbar



    - look for other view, save it's id (view_to_keep)
    - take ancestor simple_view parent
    - save parent.layout_index
    - replace gparent.children[parent.layout_index].id = view_to_keep.id

    - destroy parent <-> view_to_keep links
       parent.children[simple_view_to_keep.layout_index].id = view_to_keep.id

    - simple_view.parent = gparent;

    - destroy recursively parent

    fn destroy_view_sibling(vidx) { only keep parent_layout_index }
*/

fn destroy_view_hierarchy(editor: &mut Editor<'static>, id: view::Id) {
    let mut ids = vec![];

    {
        let v = check_view_by_id(editor, id);
        if v.is_none() {
            return;
        }
        let v = v.unwrap().clone();
        let mut v = v.write();

        for child in &mut v.children {
            ids.push(child.id);
            child.id = view::Id(0);
        }
    }

    for id in ids {
        destroy_view_hierarchy(editor, id);
    }
    dbg_println!("DESTROY view {id:?}");
    remove_view_by_id(editor, id);
}

fn get_view_parent(editor: &Editor<'static>, view: &Rc<RwLock<View<'static>>>) -> Vec<view::Id> {
    let mut v = vec![];

    let mut p_id = view.read().parent_id;
    while let Some(id) = p_id {
        v.push(id);

        if let Some(parent) = check_view_by_id(editor, id) {
            p_id = parent.read().parent_id;
        } else {
            break;
        }
    }

    v
}

/*
    NB: the text view is within a group (line/text/scrollbar)

    1st parent (p) is the text group
    2nd grand parent (pp) is the split
    3rd grand grand parent (ppp) is maybe the 1st leader or another split (not root view)

    find p sibling (p2)
    replace pp by p2 in ppp
*/
pub fn destroy_view(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // TODO(ceg): get parents
    // vec<view::Id>

    let parents = {
        let parents = get_view_parent(&editor, &view);

        dbg_println!("view.id = {:?}", view.read().id);
        dbg_println!("parents : {:?}", parents);

        parents
    };

    if parents.len() < 3 {
        return;
    }

    let to_destroy_id = {
        // get parents
        let p = get_view_by_id(&editor, parents[0]);
        let pp = get_view_by_id(&editor, parents[1]);

        let ppp = get_view_by_id(&editor, parents[2]);

        let p = p.read();
        dbg_println!("p.is_group = {}", p.is_group);
        dbg_println!("p.is_leader = {}", p.is_leader);
        dbg_println!("p.is_splittable = {}", p.is_splittable);

        let p_idx = p.layout_index.unwrap();

        let mut pp = pp.write();
        dbg_println!("pp.is_group = {}", pp.is_group);
        dbg_println!("pp.is_leader = {}", pp.is_leader);
        if !p.is_leader {
            dbg_println!("p is not leader");
            return;
        }

        // find p2 in pp

        dbg_println!("p_idx = {:?}", p_idx);

        let p2_idx: usize = if p_idx == 0 { 2 } else { 0 };

        let p_child_info = pp.children[p_idx];
        let p2_child_info = pp.children[p2_idx];

        dbg_println!("p_child_info = {:?}", p_child_info);
        dbg_println!("p2_child_info = {:?}", p2_child_info);

        let p2 = get_view_by_id(&editor, p2_child_info.id);
        let mut p2 = p2.write();

        let pp_idx = pp.layout_index.unwrap();

        let mut ppp = ppp.write();

        // remove p from pp
        pp.children.remove(p_idx);

        // replace pp by p2 in ppp
        ppp.children[pp_idx].id = p2_child_info.id;
        p2.parent_id = Some(ppp.id);
        p2.layout_index = Some(pp_idx);

        //
        pp.parent_id = None;

        p.id
    };

    destroy_view_hierarchy(editor, to_destroy_id);
}

pub static HELP_MESSAGE: &str = std::include_str!("../../../res/help_screen.txt");

pub fn help_popup(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    _view: &Rc<RwLock<View>>,
) {
    let root_view_id = editor.root_views[env.root_view_index];
    let (root_width, _root_height) = get_view_by_id(editor, root_view_id).read().dimension();

    // destroy previous
    {
        if let Some(info) = {
            get_view_by_id(editor, root_view_id)
                .write()
                .floating_children
                .pop()
        } {
            destroy_view_hierarchy(editor, info.id);
            return;
        }
    }

    let command_buffer = BufferBuilder::new(BufferKind::File)
        .buffer_name("help-pop-up")
        .internal(true)
        //           .use_buffer_log(false)
        .finalize();

    let mut pop_width = 0;
    for l in HELP_MESSAGE.lines() {
        pop_width = std::cmp::max(pop_width, l.len());
    }
    pop_width += 1;

    let pop_height = HELP_MESSAGE.lines().count();
    let x = (root_width / 2).saturating_sub(pop_width / 2);
    let y = 3; //(root_height / 2).saturating_sub(pop_height / 2);

    {
        let mut d = command_buffer.as_ref().unwrap().write();
        d.append(HELP_MESSAGE.as_bytes());
    }

    // create view
    let mut p_view = View::new(
        &mut editor,
        &mut env,
        Some(root_view_id),
        (x, y),
        (pop_width, pop_height),
        command_buffer,
        &vec!["status-mode".to_owned()],
        0,
        LayoutDirection::NotSet,
        LayoutSize::Percent { p: 100.0 },
    );

    // create corner view
    let corner_buffer = BufferBuilder::new(BufferKind::File)
        .buffer_name("corner_buffer")
        .internal(true)
        //           .use_buffer_log(false)
        .finalize();

    {
        let mut d = corner_buffer.as_ref().unwrap().write();
        d.append("◢".as_bytes()); // screen width=2
    }

    let c_view = View::new(
        &mut editor,
        &mut env,
        Some(root_view_id),
        (pop_width.saturating_sub(2 /* char width */), pop_height - 1),
        (2, 1),
        corner_buffer,
        &vec!["status-mode".to_owned()],
        0,
        LayoutDirection::NotSet,
        LayoutSize::Percent { p: 100.0 },
    );

    let c_id = c_view.id;

    p_view.floating_children.push(ChildView {
        id: c_id,
        layout_op: LayoutSize::Floating,
    });

    editor.add_view(c_view.id, c_view);

    {
        let main = get_view_by_id(editor, root_view_id);
        let mut main = main.write();

        assert_ne!(p_view.id, view::Id(0));

        main.floating_children.push(ChildView {
            id: p_view.id,
            layout_op: LayoutSize::Floating,
        });
    }

    editor.add_view(p_view.id, p_view);

    /*
    TODO(ceg): update view x, y
    lambda ?

    mode.borrow().on_view_event(
                    &mut editor,
                    &mut editor_env,
                    cb.1,
                    cb.2,
                    &ViewEvent::PreComposition,
                    Some(&mut view),
                );

    */
}
