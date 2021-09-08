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
use crate::core::view;
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
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+q" } ],    "action": "application:quit-abort" }
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
        input_map_stack.push(input_map);
    }
}

pub struct CoreMode {
    // add common filed
}
pub struct CoreModeContext {
    // add common filed
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

            dbg_println!("configure quit-abort VID {}", v.id);
            v.input_ctx.stack_pos = None;
            let input_map = build_input_event_map(CORE_QUIT_ABORT_MAP).unwrap();
            let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(input_map);
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
    // because of lifetime(editor) > lifetime(view)
    // and view.doc is a clone from editor.document_map,
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

pub fn split_with_direction(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    v: &mut View<'static>,
    width: usize,
    height: usize,
    dir: view::LayoutDirection,
    doc: &Vec<Option<Arc<RwLock<Document<'static>>>>>,
    modes: &Vec<Vec<String>>,
) {
    v.layout_direction = dir;
    let sizes = if dir == LayoutDirection::Vertical {
        view::compute_layout_sizes(height, &v.layout_ops) // options ? for ret size == 0
    } else {
        view::compute_layout_sizes(width, &v.layout_ops) // options ? for ret size == 0
    };

    dbg_println!(
        "SPLIT WITH DIRECTION {:?} = SIZE {:?} NB OPS {}",
        dir,
        sizes,
        v.layout_ops.len()
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

        // vertically
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
                return;
            }
        };

        view.layout_index = Some(idx);

        // move this after call
        // focus on first child ? // check again clipping code
        if idx == 0 {
            env.focus_changed_to = Some(view.id); // post input
        }

        let id = view.id;
        v.children.push(id);
        let rc = Rc::new(RwLock::new(view));
        editor.view_map.insert(id, Rc::clone(&rc));

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
    view_ids: &Vec<view::Id>,
) {
    let parent = editor.view_map.get(&parent_id).unwrap();
    let mut parent = parent.write();

    parent.layout_direction = dir;
    let sizes = if dir == LayoutDirection::Vertical {
        view::compute_layout_sizes(height, &parent.layout_ops) // options ? for ret size == 0
    } else {
        view::compute_layout_sizes(width, &parent.layout_ops) // options ? for ret size == 0
    };

    dbg_println!(
        "LAYOUT WITH DIRECTION {:?} = SIZE {:?} NB OPS {}",
        dir,
        sizes,
        parent.layout_ops.len()
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
        // focus on first child ? // check again clipping code
        if idx == 0 {
            env.focus_changed_to = Some(view.id); // post input
        }

        let id = view.id;
        parent.children.push(id);

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

fn find_first_splitable_parent(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> Option<view::Id> {
    let mut start_id = { view.read().id };

    loop {
        let view = editor.view_map.get(&start_id)?;
        let v = view.read();
        if v.is_group_leader {
            return Some(v.id);
        }
        start_id = v.parent_id?;
    }
}

/*
  TODO(ceg):  create view + modes etc ... link parents, set layout rules

         parent
            |
           v1

create new parent: p2
create new vertical splitter:    splitter

         parent  ,   p2
            |
           v1        splitter

clone v1 -> v2

           parent   , p2, splitter
            |
           v1       , v2

cut parent <-> v1,    set parent <-> p2

           parent
             |
             p2

      v1  , splitter   , v2

build_layout  for p2

            parent
              |
             p2
           / | \
         /  |   \
      /    |     \
    v1  splitter  v2



    look for first view with the is_group_leader flag set

    create a new_parent with width height

    create a new_view with same params/modes

    update v.parent and siblings  id...

    split_with_direction -> layout_view_ids_with_directions()

    layout_view_ids_with_direction(&mut editor,
        &mut env,
        &mut v,
        width,
        height,
        LayoutDirection::Horizontal,
        Vec<view::Id>);

*/

pub fn split_view_with_direction(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    dir: view::LayoutDirection,
) {
    let id = find_first_splitable_parent(editor, env, view);
    if id.is_none() {
        return;
    }
    let id = id.unwrap();

    let view = editor.view_map.get(&id);
    let view = view.unwrap().clone();

    let (v_id, parent_id, x, y, width, height, doc, original_modes, layout_index) = {
        let v = view.read();

        dbg_println!("SPLITTING {:?} VID {}", dir, v.id);

        let (width, height) = {
            let screen = v.screen.read();
            (screen.width(), screen.height())
        };

        // compute left and right size as current View / 2
        // get screen

        let document_map = editor.document_map.clone();
        let document_map = document_map.read();

        let doc = {
            if v.document.is_none() {
                None
            } else {
                let doc_id = v.document().unwrap();
                let doc_id = doc_id.read().id;
                if let Some(_doc) = document_map.get(&doc_id) {
                    let doc = document_map.get(&doc_id).unwrap().clone();
                    Some(Arc::clone(&doc))
                } else {
                    None
                }
            }
        };

        (
            v.id,
            v.parent_id,
            v.x,
            v.y,
            width,
            height,
            doc,
            v.modes.clone(),
            v.layout_index,
        )
    };

    // create new parent p2
    let mut p2 = View::new(
        &mut editor,
        &mut env,
        parent_id,
        x, // relative to parent, i32 allow negative moves?
        y, // relative to parent, i32 allow negative moves?
        width,
        height,
        None,
        &vec![], // TODO(ceg): add core mode fr save/quit/quit/abort/split{V,H}
        0,
    );

    // children_layout_and_modes
    let ops_modes = vec![
        LayoutOperation::Percent { p: 50.0 },
        LayoutOperation::Fixed { size: 1 },
        LayoutOperation::RemainPercent { p: 100.0 },
    ];

    p2.layout_ops = ops_modes;
    let p2_id = p2.id;

    let rc = Rc::new(RwLock::new(p2));
    editor.view_map.insert(p2_id, Rc::clone(&rc)); // move to View::new

    if let Some(parent_id) = parent_id {
        if let Some(gp) = editor.view_map.get(&parent_id) {
            let mut gp = gp.write();
            if let Some(layout_index) = layout_index {
                gp.children[layout_index] = p2_id;
            }
        }
    }

    let splitter_mode = match dir {
        view::LayoutDirection::Horizontal => vec!["vsplit-mode".to_owned()],
        view::LayoutDirection::Vertical => vec!["hsplit-mode".to_owned()],
        _ => panic!(),
    };

    // create splitter
    let splitter = View::new(
        &mut editor,
        &mut env,
        Some(p2_id),
        x, // relative to parent, i32 allow negative moves?
        y, // relative to parent, i32 allow negative moves?
        width,
        height,
        None,
        &splitter_mode,
        0,
    );

    let splitter_id = splitter.id;
    let rc = Rc::new(RwLock::new(splitter));
    editor.view_map.insert(splitter_id, Rc::clone(&rc)); // move to View::new

    // create new parent v2
    let v2 = View::new(
        &mut editor,
        &mut env,
        Some(p2_id),
        x, // relative to parent, i32 allow negative moves?
        y, // relative to parent, i32 allow negative moves?
        width,
        height,
        doc.clone(),
        &original_modes,
        0,
    );
    let v2_id = v2.id;
    let rc = Rc::new(RwLock::new(v2));
    editor.view_map.insert(v2_id, Rc::clone(&rc)); // move to View::new

    let view_ids = vec![v_id, splitter_id, v2_id];

    layout_view_ids_with_direction(&mut editor, &mut env, p2_id, width, height, dir, &view_ids);
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

// quit hack ignoring other children
pub fn increase_layout_op(
    op: LayoutOperation,
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

    let new_op = match op {
        LayoutOperation::Fixed { size } if size < max_size => {
            LayoutOperation::Fixed { size: size + 1 }
        }
        LayoutOperation::Percent { p } => {
            if cur_size + diff >= max_size {
                return op;
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
        _ => op,
    };

    dbg_println!("INC LAYOUT NEW OP {:?}", new_op);

    new_op
}

// quit hack ignoring other children
pub fn decrease_layout_op(
    op: LayoutOperation,
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

    let new_op = match op {
        LayoutOperation::Fixed { size } if size > diff => {
            LayoutOperation::Fixed { size: size - diff }
        }
        LayoutOperation::Percent { p } => {
            if cur_size <= diff {
                return op;
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
                return op;
            }
            LayoutOperation::RemainMinus {
                minus: ((minus * 100 + max_size) / 100) + 1,
            }
        }
        _ => op,
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
    let new_op = decrease_layout_op(pv.layout_ops[lidx], max_size, max_size, 1);
    pv.layout_ops[lidx] = new_op;
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
    let new_op = increase_layout_op(pv.layout_ops[lidx], max_size, cur_size, 1);
    pv.layout_ops[lidx] = new_op;
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
    let new_op = increase_layout_op(pv.layout_ops[lidx], max_size, cur_size, 1);
    pv.layout_ops[lidx] = new_op;
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
    let new_op = decrease_layout_op(pv.layout_ops[lidx], max_size, cur_size, 1);
    pv.layout_ops[lidx] = new_op;
}

/*
    FIXME(ceg): broken, rewrite this, new is_group_leader flag not handled
    TODO(ceg): document this function

    add view.is_group_leader = true aka is_group_leader


                     ggparent
                        |
                     gparent
                        |
             __________|_____________________
            |                                \
         group_leader (1)    splitter     group_leader (2)
           /  |    \                          /  |      \
         /   |      \                       /    |       \
      /     |        \                   /       |        \
    lines text_view  vscrollbar           lines text_view  vscrollbar

                     ggparent
                        |
                     gparent
                        |
                        |
                 group_leader (1)
                    /  |    \
                  /   |      \
               /     |        \
             lines text_view  vscrollbar



    (1) look for sibling group leader

    save it's parent_layout_index

    (2) look for first ancestor (gparent) with the is_group_leader flag set if none -> return
    save gparent_layout_index

    ------

    (3) destroy all gparent children except parent

    fn destroy_view_sibling(vidx) { only keep parent_layout_index }


    ggparent.children[gparent_layout_index] = parent.id

    destroy gparent

*/

pub fn destroy_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv,
    view: &Rc<RwLock<View<'static>>>,
) {
    // current view/id
    let v = view.write();

    if v.destroyable == false {
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

    let v_layout_index = v.layout_index.unwrap();

    let mut destroy = vec![];

    // get parent view/id
    let pvid = *v.parent_id.as_ref().unwrap();
    let pv = editor.view_map.get(&pvid).unwrap().clone();
    let mut pv = pv.write();

    if pv.children.len() != 3 {
        dbg_println!(" pv.children.len({}) != 3", pv.children.len());
        // not handled yet
        return;
    }

    if let Some(ppvid) = pv.parent_id {
        // get grand parent view/id
        let ppv = editor.view_map.get(&ppvid).unwrap().clone();
        let mut ppv = ppv.write();

        let pv_layout_index = pv.layout_index.unwrap();

        let mut kept_vid = None;

        // TODO(ceg): get sibling ids
        // mark siblings for delete
        for (idx, view_id) in pv.children.iter().enumerate() {
            if idx == v_layout_index {
                dbg_println!("prepare delete of view id {}", *view_id);
                destroy.push(*view_id);
            } else if idx == 1 {
                // separator index
                // TODO(ceg): add view_kind ? text/scrollbar/hsplit/vsplit etc ?
                dbg_println!("prepare delete of view id {} (separator)", *view_id);
                destroy.push(*view_id);
            } else {
                dbg_println!("keep view id {}", *view_id);
                kept_vid = Some(*view_id);
            }
        }

        if let Some(kept_vid) = kept_vid {
            // replace parent in grand-parent
            ppv.children[pv_layout_index] = kept_vid;
            pv.parent_id = Some(ppvid);
            // update grand parent focus: // TODO(ceg): find a better way
            ppv.focus_to = Some(kept_vid);

            // update link to grand-parent  (new parent)
            let kept_v = editor.view_map.get(&kept_vid).unwrap().clone();
            let mut kept_v = kept_v.write();
            kept_v.parent_id = Some(ppvid);
            kept_v.layout_index = Some(pv_layout_index);

            kept_v.destroyable = pv.destroyable; // NB: take parent policy

            dbg_println!("prepare delete of view id {} (parent)", pvid);
            dbg_println!("set focus to view id {}", kept_vid);
            destroy.push(pvid);
            env.focus_changed_to = Some(kept_vid); // post input
        }
    } else {
        // TODO(ceg): get sibling ids
        // mark self+siblings for delete
        let mut kept_vid = None;

        for (idx, view_id) in pv.children.iter().enumerate() {
            if idx == v_layout_index {
                dbg_println!("prepare delete of view id {}", *view_id);
                destroy.push(*view_id);
            } else if idx == 1 {
                // separator index
                // TODO(ceg): add view_kind ? text/scrollbar/hsplit/vsplit etc ?
                dbg_println!("prepare delete of view id {} (separator)", *view_id);
                destroy.push(*view_id);
            } else {
                dbg_println!("keep view id {}", *view_id);
                kept_vid = Some(*view_id);
            }
        }

        if let Some(kept_vid) = kept_vid {
            dbg_println!("root view update");
            dbg_println!("delete {}", pvid);
            destroy.push(pvid);

            for i in 0..editor.root_views.len() {
                if editor.root_views[i] == pvid {
                    dbg_println!("update root view slot {}", i);

                    editor.root_views[i] = kept_vid;
                    env.view_id = kept_vid;
                    break;
                }
            }

            let kept_v = editor.view_map.get(&kept_vid).unwrap().clone();
            let mut kept_v = kept_v.write();
            kept_v.parent_id = None;
            kept_v.layout_index = None;

            env.focus_changed_to = Some(kept_vid); // post input
        }
    };

    dbg_println!("destroy view(s) {:?}", destroy);
    for vid in destroy {
        editor.view_map.remove(&vid);
    }
}
