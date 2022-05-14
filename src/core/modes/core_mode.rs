use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;
use std::sync::Arc;

use super::Mode;

use crate::core::document::Document;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;

use crate::core::document::DocumentBuilder;

use crate::core::view;
use crate::core::view::ChildView;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;
use crate::core::view::View;

static CORE_INPUT_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "F4"     }],                          "action": "toggle-debug-print" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+s" } ],    "action": "save-document" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+c" } ],    "action": "application:quit" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+q" } ],    "action": "application:quit-abort" },
     { "in": [{ "key": "ctrl+p" } ],                         "action": "help-pop-up" }

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

        register_input_stage_action(&mut map, "save-document", save_document); // core ?
        register_input_stage_action(&mut map, "split-vertically", split_vertically);
        register_input_stage_action(&mut map, "split-horizontally", split_horizontally);
        register_input_stage_action(&mut map, "destroy-view", destroy_view);

        register_input_stage_action(&mut map, "increase-left", increase_left);
        register_input_stage_action(&mut map, "decrease-left", decrease_left);
        register_input_stage_action(&mut map, "increase-right", increase_right);
        register_input_stage_action(&mut map, "decrease-right", decrease_right);
    }
}

// Mode "core"
pub fn application_quit(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // TODO(ceg): change this
    // editor.changed_doc : HashSet<document::Id>
    // if editor.change_docs.len() != 0

    let doc = { view.read().document().unwrap() };
    let doc = doc.read();
    if !doc.changed {
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
    let status_vid = view::get_status_view(&editor, &env, view);

    dbg_println!("DOC CHANGED !\n");
    dbg_println!("STATUS VID = {:?}", status_vid);

    if let Some(svid) = status_vid {
        let status_view = editor.view_map.get(&svid).unwrap();
        //
        let doc = status_view.read().document().unwrap();
        let mut doc = doc.write();
        // clear doc
        let sz = doc.size();
        doc.remove(0, sz, None);
        // set status text
        let text = "Modified documents exist. Really quit? y/n";
        let bytes = text.as_bytes();
        doc.insert(0, bytes.len(), &bytes);

        // push new input map for y/n
        {
            let mut v = view.write();
            // lock focus on v
            // env.focus_locked_on = Some(v.id);

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
        // env.focus_locked_on = None;
    }

    // reset status view : TODO(ceg): view::reset_status_view(&editor, view);
    let status_vid = view::get_status_view(&editor, &env, view);
    if let Some(status_vid) = status_vid {
        let status_view = editor.view_map.get(&status_vid).unwrap();
        let doc = status_view.read().document().unwrap();
        let mut doc = doc.write();
        // clear buffer
        let sz = doc.size();
        doc.remove(0, sz, None);
    }
}

pub fn toggle_dgb_print(_editor: &mut Editor, _env: &mut EditorEnv, _view: &Rc<RwLock<View>>) {
    crate::core::toggle_dbg_println();
}

pub fn save_document(editor: &mut Editor<'static>, _env: &mut EditorEnv, view: &Rc<RwLock<View>>) {
    let v = view.write();

    let doc_id = {
        let doc = v.document().unwrap();
        {
            // - needed ? already syncing ? -
            let doc = doc.read();
            if !doc.changed || doc.is_syncing {
                // TODO(ceg): ensure all other places are checking this flag, all doc....write()
                // better, some permissions mechanism ?
                // doc.access_permissions = r-
                // doc.access_permissions = -w
                // doc.access_permissions = rw
                return;
            }
        }

        // - set sync flag -
        {
            let mut doc = doc.write();
            let doc_id = doc.id;
            doc.is_syncing = true;
            doc_id
        }
    };

    // - send sync job to worker -
    //
    // NB: We must take the doc clone from Editor not View
    // because lifetime(editor) >= lifetime(view)
    // ( view.doc is a clone from editor.document_map ),
    // doing this let us avoid the use manual lifetime annotations ('static)
    // and errors like "data from `view` flows into `editor`"
    let document_map = editor.document_map.clone();
    let document_map = document_map.read();

    if let Some(doc) = document_map.get(&doc_id) {
        let msg = EventMessage {
            seq: 0,
            event: Event::SyncTask {
                doc: Arc::clone(doc),
            },
        };
        editor.worker_tx.send(msg).unwrap_or(());
    }
}

// NB:
//     a  Vertical split   <-> LayoutDirection::Horizontal <-> Left-To-Right
//     an Horizontal split <-> LayoutDirection::Vertical   <-> Top-To-Bottom
pub fn split_with_direction(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    v: &mut View<'static>,
    width: usize,
    height: usize,
    dir: view::LayoutDirection,
    layout_ops: &Vec<LayoutOperation>,
    doc: &Vec<Option<Arc<RwLock<Document<'static>>>>>,
    modes: &Vec<Vec<String>>,
) {
    v.layout_direction = dir;
    let sizes = if dir == LayoutDirection::Vertical {
        view::compute_layout_sizes(height, &layout_ops) // options ? for ret size == 0
    } else {
        view::compute_layout_sizes(width, &layout_ops) // options ? for ret size == 0
    };

    dbg_println!(
        "SPLIT WITH DIRECTION {:?} = SIZE {:?} NB OPS {}",
        dir,
        sizes,
        layout_ops.len()
    );

    let mut x = v.x;
    let mut y = v.y;

    for (idx, size) in sizes.iter().enumerate() {
        let size = std::cmp::max(1, *size); // screen require 1x1 as min
        let (width, height) = match dir {
            LayoutDirection::Vertical => (width, size),
            LayoutDirection::Horizontal => (size, height),
            _ => {
                return;
            }
        };

        // allocate the view
        let mut view = match dir {
            LayoutDirection::Vertical | LayoutDirection::Horizontal => View::new(
                &mut editor,
                &mut env,
                Some(v.id),
                x,
                y,
                width,
                height,
                doc[idx].clone(),
                &modes[idx],
                v.start_offset,
            ),

            _ => {
                // panic!
                return;
            }
        };

        view.layout_index = Some(idx);

        // move this after call
        // focus on first child ? // check again clipping code

        dbg_println!("ALLOCATE new : {:?}", view.id);

        let id = view.id;
        v.children.push(ChildView {
            layout_op: layout_ops[idx].clone(),
            id,
        });

        let view = Rc::new(RwLock::new(view));
        editor.view_map.insert(id, Rc::clone(&view));

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

pub fn layout_view_ids_with_direction(
    editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    parent_id: view::Id,
    width: usize,
    height: usize,
    dir: view::LayoutDirection,
    layout_ops: &Vec<LayoutOperation>,
    view_ids: &Vec<view::Id>,
) {
    let parent = editor.view_map.get(&parent_id).unwrap();
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

        let view = editor.view_map.get(&view_ids[idx]).unwrap();
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
        let view = editor.view_map.get(&start_id)?;
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
    doc: Option<Arc<RwLock<Document<'static>>>>,
    original_modes: Vec<String>,
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
        doc: v.document().clone(),
        original_modes: v.modes.clone(),
        layout_index: v.layout_index,
    }
}

pub fn split_view_with_direction(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    dir: view::LayoutDirection,
) {
    let id = find_first_splittable_parent(editor, env, view); // group leader (ex: simple_view)
    if id.is_none() {
        return;
    }
    let id = id.unwrap();

    let view = editor.view_map.get(&id);
    let view = view.unwrap().clone();

    let split_info = build_split_info(&view, dir);

    // create new parent (will replace [view_to_split] in the hierarchy)
    let mut new_parent = View::new(
        &mut editor,
        &mut env,
        split_info.parent_id, // group leader's parent
        split_info.x,         // relative to parent, i32 allow negative moves?
        split_info.y,         // relative to parent, i32 allow negative moves?
        split_info.width,
        split_info.height,
        None,
        &vec![], // TODO(ceg): add core mode fr save/quit/quit/abort/split{V,H}
        0,
    );

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
                return;
            }
        }
        view::LayoutDirection::Vertical => {
            if split_info.height <= HEIGHT_MIN {
                dbg_println!(
                    "view to split not wide enough : height {} <= {}",
                    split_info.height,
                    HEIGHT_MIN
                );
                return;
            }
        }

        _ => {
            return;
        }
    }

    // children_layout_and_modes
    let layout_ops = vec![
        LayoutOperation::Percent { p: 50.0 }, // left (view_to_split)
        LayoutOperation::Fixed { size: 1 },   // splitter
        LayoutOperation::RemainPercent { p: 100.0 }, // right (view_clone)
    ];

    // new parent replaces view_to_split i parent(view_to_split)
    new_parent.layout_index = split_info.layout_index;
    let new_parent_id = new_parent.id;
    // insert new_parent into editor global map
    editor.add_view(new_parent_id, Rc::new(RwLock::new(new_parent)));

    dbg_println!("new parent = {:?}", new_parent_id);

    // update grand parent, replace v1_id by p2_id
    if let Some(parent_id) = split_info.parent_id {
        if let Some(gp) = editor.view_map.get(&parent_id) {
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
            split_info.x, // relative to parent, i32 allow negative moves?
            split_info.y, // relative to parent, i32 allow negative moves?
            split_info.width,
            split_info.height,
            None,
            &splitter_mode,
            0,
        );

        let splitter_id = splitter.id;
        editor.add_view(splitter_id, Rc::new(RwLock::new(splitter)));

        dbg_println!("splitter_id = {:?}", splitter_id);

        splitter_id
    };

    // create view clone
    let view_clone_id = {
        let view_clone = View::new(
            &mut editor,
            &mut env,
            Some(new_parent_id),
            split_info.x, // relative to parent, i32 allow negative moves?
            split_info.y, // relative to parent, i32 allow negative moves?
            split_info.width,
            split_info.height,
            split_info.doc.clone(),
            &split_info.original_modes,
            0,
        );
        let view_clone_id = view_clone.id;
        editor.add_view(view_clone_id, Rc::new(RwLock::new(view_clone)));

        dbg_println!("view_clone_id = {:?}", view_clone_id);

        view_clone_id
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
    op: &LayoutOperation,
    max_size: usize,
    cur_size: usize,
    diff: usize,
) -> LayoutOperation {
    dbg_println!(
        "INC LAYOUT OP {:?}, max_size = {} max_size, cur_size {} diff {}",
        op,
        max_size,
        cur_size,
        diff
    );

    let new_op = match *op {
        LayoutOperation::Fixed { size } if size < max_size => {
            LayoutOperation::Fixed { size: size + 1 }
        }
        LayoutOperation::Percent { p } => {
            if cur_size + diff >= max_size {
                return op.clone();
            }
            let expect_p = ((cur_size + diff) as f32 * p) / cur_size as f32;
            dbg_println!("LAYOUT expect_p = {}", expect_p);
            LayoutOperation::Percent { p: expect_p }
        }
        LayoutOperation::RemainPercent { p } if p < 99.0 => {
            let unit = max_size as f32 / 100.0;
            LayoutOperation::RemainPercent { p: p + unit }
        }
        LayoutOperation::RemainMinus { minus } => {
            dbg_println!(
                "LAYOUT = max_size{} - minus*100{} / 100 = {}",
                minus * 100,
                max_size,
                max_size.saturating_sub(minus * 100) / 100
            );
            LayoutOperation::RemainMinus {
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
    op: &LayoutOperation,
    // TODO(ceg): min_size: usize,
    max_size: usize,
    cur_size: usize,
    diff: usize, // decrease amount
) -> LayoutOperation {
    dbg_println!(
        "DEC LAYOUT OP {:?}, max_size = {} max_size, cur_size {} diff {}",
        op,
        max_size,
        cur_size,
        diff
    );

    let new_op = match *op {
        LayoutOperation::Fixed { size } if size > diff => {
            LayoutOperation::Fixed { size: size - diff }
        }
        LayoutOperation::Percent { p } => {
            if cur_size <= diff {
                return op.clone();
            }

            let expect_p = ((cur_size - diff) as f32 * p) / cur_size as f32;
            dbg_println!("LAYOUT expect_p = {}", expect_p);
            LayoutOperation::Percent { p: expect_p }
        }

        LayoutOperation::RemainPercent { p } if p > 2.0 => {
            let unit = (max_size as f32) / 100.0;
            LayoutOperation::RemainPercent { p: p - unit }
        }
        LayoutOperation::RemainMinus { minus } => {
            dbg_println!(
                "LAYOUT = minus * 100 {} + max_size {} / 100 = {}",
                minus * 100,
                max_size,
                (minus * 100 + max_size) / 100
            );
            if ((minus * 100 + max_size) / 100) + 1 > 100 {
                return op.clone();
            }
            LayoutOperation::RemainMinus {
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
    let pv = editor.view_map.get(&pvid).unwrap();
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
    let pv = editor.view_map.get(&pvid).unwrap();
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
    let pv = editor.view_map.get(&pvid).unwrap();
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
    let pv = editor.view_map.get(&pvid).unwrap();
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
        let v = editor.view_map.get(&id);
        if v.is_none() {
            return;
        }
        let v = v.unwrap().clone();
        let mut v = v.as_ref().write();

        for child in &mut v.children {
            ids.push(child.id);
            child.id = view::Id(0);
        }
    }

    for id in ids {
        destroy_view_hierarchy(editor, id);
    }
    dbg_println!("DESTROY view {id:?}");
    editor.view_map.remove(&id);
}

pub fn destroy_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let to_destroy_id = {
        dbg_println!(">>> DESTROY -------------------");
        // current view/id

        let v = view.read();

        dbg_println!("-- DESTROY VIEW {:?}", v.id);

        if !v.destroyable {
            return;
        }

        // check parent
        if v.parent_id.is_none() {
            // nothing to do
            // check root_views presence
            dbg_println!("No parent, ignore");
            return;
        }

        // no index in parent : not a split, etc..
        if v.layout_index.is_none() {
            dbg_println!("No layout index found, ignore");
            return;
        }

        // get PARENT
        let p_id = v.parent_id;
        let p_id = p_id.unwrap();

        dbg_println!("-- DESTROY VEW : PARENT {:?}", p_id);
        let v_p = editor.view_map.get(&p_id);
        if v_p.is_none() {
            return;
        }
        let v_p = v_p.unwrap();
        let v_p = v_p.write();

        // get PARENT PARENT
        let pp_id = v_p.parent_id.unwrap();
        dbg_println!("-- DESTROY VIEW: PARENT PARENT {:?}", pp_id);

        let v_pp = editor.view_map.get(&pp_id);
        if v_pp.is_none() {
            return;
        }
        let v_pp = v_pp.unwrap().clone();
        let mut v_pp = v_pp.as_ref().write();
        if v_pp.destroyable == false {
            dbg_println!("-- DESTROY : TOP VIEW REACHED");
            return;
        }

        // get PARENT PARENT PARENT
        let ppp_id = v_pp.parent_id.unwrap();
        dbg_println!("-- DESTROY PARENT PARENT VIEW {:?}", ppp_id);

        let v_ppp = editor.view_map.get(&ppp_id);
        if v_ppp.is_none() {
            return;
        }
        let v_ppp = v_ppp.unwrap().clone();
        let mut v_ppp = v_ppp.as_ref().write();

        let v_p_layout_index = v_p.layout_index;
        dbg_println!("DESTROY p_layout_index = {v_p_layout_index:?}");
        let keep_layout_index = match v_p_layout_index {
            None => return,
            Some(0) => 2,
            Some(2) => 0,
            _ => panic!("invalid configuration"),
        };

        dbg_println!("DESTROY keep_layout_index = {keep_layout_index}");

        let v_to_keep_id = v_pp.children[keep_layout_index].id;

        v_pp.children[keep_layout_index].id = view::Id(0);

        dbg_println!("DESTROY v_to_keep_id = {v_to_keep_id:?}");

        // simple test erase links
        // get view_to_keep id
        let v_pp_layout_index = v_pp.layout_index;
        let v_p_layout_index = v_p.layout_index;

        let to_destroy = v_ppp.children[v_pp_layout_index.unwrap()].id;

        v_pp.children[v_p_layout_index.unwrap()].id = view::Id(0); // removed

        // replace
        v_ppp.children[v_pp_layout_index.unwrap()].id = v_to_keep_id;

        let to_keep = editor.view_map.get(&v_to_keep_id);
        if to_keep.is_none() {
            return;
        }
        let to_keep = to_keep.unwrap().clone();
        let mut to_keep = to_keep.as_ref().write();
        to_keep.layout_index = v_pp_layout_index;
        to_keep.parent_id = Some(ppp_id);

        dbg_println!("-- DESTROY to_destroy {to_destroy:?}");

        assert_eq!(to_destroy, pp_id);

        // TODO: find in v_to_keep_id hierarchy the 1st child with a grab input flag
        // editor.find_first_child_view(parent, |&v| { v.grab == true });
        // env.recompute_focus = true;

        pp_id
    };

    destroy_view_hierarchy(editor, to_destroy_id);
}

static HELP_MESSAGE: &str = r#"-*- Welcome to unlimitED! -*-

unlimitED! is an experimental text editor (running in the terminal).


SYNOPSIS
unlimited [options] [file ..]


It comes with:

  - basic UTF-8 support
  - very large file support
  - "infinite" undo/redo
  - multi-cursors
  - mouse selection (graphical terminal)

[Quit]
    Quit:           => ctrl+x ctrl+c

    Quit (no save)  => ctrl+x ctrl+q

    NB: quit will wait for large file(s) sync to storage.


[Moves]
    Left            =>
    Right           =>
    Up              =>
    Down            =>


[Edit]
    ctrl+o          => Open file (TODO)

    ctrl+u          => Undo
    ctrl+r          => Redo

[Selection/Copy/Paste]
    with the keyboard:

    with the mouse (X11 terminal):

[Save]
    ctrl+x ctrl+s   => Save
                    synchronization of large file(s) is done in the background and does not block the ui.



[Document Selection]



NB: unlimitED! comes with ABSOLUTELY NO WARRANTY
"#;

pub fn help_popup(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    _view: &Rc<RwLock<View>>,
) {
    let main_vid = view::Id(1);

    let (main_width, main_height) = {
        let main = editor.view_map.get(&main_vid).unwrap().read();
        (main.width, main.height)
    };

    // destroy previous
    {
        if let Some(info) = {
            let mut main = editor.view_map.get(&main_vid).unwrap().write();
            main.floating_children.pop()
        } {
            editor.view_map.remove(&info.id);
            return;
        }
    }

    let command_doc = DocumentBuilder::new()
        .document_name("help-pop-up")
        .internal(true)
        //           .use_buffer_log(false)
        .finalize();

    let pop_height = 25;
    let pop_width = main_width.saturating_sub(1);
    let x = (main_width / 2).saturating_sub(pop_width / 2);
    let y = (main_height / 2).saturating_sub(pop_height / 2);

    {
        let mut d = command_doc.as_ref().unwrap().write();
        d.append(HELP_MESSAGE.as_bytes());
    }

    // create view
    let p_view = View::new(
        &mut editor,
        &mut env,
        Some(main_vid),
        x,
        y,
        pop_width,
        pop_height,
        command_doc,
        &vec!["status-mode".to_owned()],
        0,
    );

    {
        let mut main = editor.view_map.get(&main_vid).unwrap().write();

        main.floating_children.push(ChildView {
            id: p_view.id,
            layout_op: LayoutOperation::Floating,
        });
    }

    editor.add_view(p_view.id, Rc::new(RwLock::new(p_view)));

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
