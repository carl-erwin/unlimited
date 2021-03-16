use std::any::Any;
use std::cell::RefCell;

use std::rc::Rc;
use std::sync::Arc;
use std::sync::RwLock;

use super::Mode;

use crate::core::document::Document;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::view;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;
use crate::core::view::View;

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
        register_input_stage_action(&mut map, "application:quit", application_quit);
        register_input_stage_action(&mut map, "application:quit-abort", application_quit_abort);
        register_input_stage_action(&mut map, "save-document", save_document); // core ?
        register_input_stage_action(&mut map, "split-vertically", split_vertically);
        register_input_stage_action(&mut map, "split-horizontally", split_horizontally);
        register_input_stage_action(&mut map, "destroy-view", destroy_view);
    }
}

// Mode "core"
pub fn application_quit(_editor: &mut Editor, env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = &view.borrow();
    let doc = v.document.as_ref().unwrap();
    let doc = doc.as_ref().read().unwrap();

    if !doc.changed {
        env.quit = true;
    }
}

pub fn application_quit_abort(
    _editor: &mut Editor,
    env: &mut EditorEnv,

    _view: &Rc<RefCell<View>>,
) {
    env.quit = true;
}

pub fn save_document(editor: &mut Editor, _env: &mut EditorEnv, view: &Rc<RefCell<View>>) {
    let v = view.borrow_mut();

    let doc_id = {
        let doc = v.document.as_ref().unwrap();
        {
            // - needed ? already syncing ? -
            let doc = doc.as_ref().read().unwrap();
            if !doc.changed || doc.is_syncing {
                // TODO: ensure all over places are checking this flag, all doc....write()
                // better, some permissions mechanism ?
                // doc.access_permissions = r-
                // doc.access_permissions = -w
                // doc.access_permissions = rw
                return;
            }
        }

        // - set sync flag -
        {
            let mut doc = doc.as_ref().write().unwrap();
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
    let document_map = document_map.read().unwrap();

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
    let sizes = if dir == LayoutDirection::Vertical {
        view::compute_layout_sizes(width, &v.layout_ops) // options ? for ret size == 0
    } else {
        view::compute_layout_sizes(height, &v.layout_ops) // options ? for ret size == 0
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
            LayoutDirection::Vertical => (size, height),
            LayoutDirection::Horizontal => (width, size),
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
        let rc = Rc::new(RefCell::new(view));
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

pub fn split_vertically(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RefCell<View<'static>>>,
) {
    let mut v = view.borrow_mut();

    // check if already split
    if v.children.len() != 0 {
        return;
    }

    // compute left and right size as current View / 2
    // get screen

    let (width, height) = {
        let screen = v.screen.read().unwrap();
        (screen.width(), screen.height())
    };

    let document_map = editor.document_map.clone();
    let document_map = document_map.read().unwrap();

    let doc = {
        if v.document.is_none() {
            None
        } else {
            let doc_id = v.document.as_ref().unwrap();
            let doc_id = doc_id.read().unwrap().id;
            if let Some(_doc) = document_map.get(&doc_id) {
                let doc = document_map.get(&doc_id).unwrap().clone();
                Some(Arc::clone(&doc))
            } else {
                None
            }
        }
    };

    let parent_modes: Vec<String> = v.mode_ctx.iter().map(|(name, _)| name.clone()).collect();

    // children_layout_and_modes
    let ops_modes = vec![
        (
            LayoutOperation::Percent { p: 50 },
            doc.clone(),
            parent_modes.clone(),
        ),
        // separator, will crash no text hard coded in compositing stage
        // TODO: per view action map
        (
            LayoutOperation::Fixed { size: 1 },
            None,
            vec!["vsplit-mode".to_owned()],
        ),
        (
            LayoutOperation::RemainPercent { p: 100 },
            doc.clone(),
            parent_modes.clone(),
        ),
    ];

    v.layout_direction = LayoutDirection::Vertical;
    v.layout_ops = ops_modes.iter().map(|e| e.0.clone()).collect();
    let docs = ops_modes.iter().map(|e| e.1.clone()).collect();
    let modes = ops_modes.iter().map(|e| e.2.clone()).collect();

    split_with_direction(
        &mut editor,
        &mut env,
        &mut v,
        width,
        height,
        LayoutDirection::Vertical,
        &docs,
        &modes,
    );

    /*
     TODO
         + swap children[0]
    */
}

pub fn split_horizontally(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RefCell<View<'static>>>,
) {
    let mut v = view.borrow_mut();

    // check if already split
    if v.children.len() != 0 {
        return;
    }

    // compute left and right size as current View / 2
    // get screen

    let (width, height) = {
        let screen = v.screen.read().unwrap();
        (screen.width(), screen.height())
    };

    let doc = {
        if v.document.is_none() {
            None
        } else {
            let document_map = editor.document_map.clone();
            let document_map = document_map.read().unwrap();

            let doc_id = v.document.as_ref().unwrap();
            let doc_id = doc_id.read().unwrap().id;
            if let Some(_doc) = document_map.get(&doc_id) {
                let doc = document_map.get(&doc_id).unwrap().clone();
                Some(Arc::clone(&doc))
            } else {
                None
            }
        }
    };

    let parent_modes: Vec<String> = v.mode_ctx.iter().map(|(name, _)| name.clone()).collect();

    // children_layout_and_modes
    let ops_modes = vec![
        (
            LayoutOperation::Percent { p: 50 },
            doc.clone(),
            parent_modes.clone(),
        ),
        // separator, will crash no text hard coded in compositing stage
        // TODO: per view action map
        (
            LayoutOperation::Fixed { size: 1 },
            None,
            vec!["hsplit-mode".to_owned()],
        ),
        (
            LayoutOperation::RemainPercent { p: 100 },
            doc.clone(),
            parent_modes.clone(),
        ),
    ];

    v.layout_direction = LayoutDirection::Horizontal;
    v.layout_ops = ops_modes.iter().map(|e| e.0.clone()).collect();
    let docs = ops_modes.iter().map(|e| e.1.clone()).collect();
    let modes = ops_modes.iter().map(|e| e.2.clone()).collect();

    split_with_direction(
        &mut editor,
        &mut env,
        &mut v,
        width,
        height,
        LayoutDirection::Horizontal,
        &docs,
        &modes,
    );

    /*
     TODO
         + swap children[0]
    */
}

/*
   TODO: must destroy/swap hierarchy, gparent/root_view
   rapid hack no hierarchy update
   partial destroy

        if !pv {
            remove from root_view
            keep at least 1 view
        }

        1) scan siblings
             saturating_sub(1) + sort();
             pv[v.layout_index-1] == separator -> kill pv.children index list
             pv[v.layout_index+1] == separator -> kill pv.children index list
             pv[v.layout_index]   ==  self     -> kill pv.children index list

        if pv.children.len() == 1 swap remain_idx in grand-parent (ppv)  // TODO: if p.no_destroyed ( need special command to "close document" not destroy view)
            if ! pvv {
                replace pv from root_view[]
            }
            else {
                ppv.children[ pv.layout_index ] -> kill vid list;
                ppv.children[ pv.layout_index ] = remain_vid;
            }
*/
pub fn destroy_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv,
    view: &Rc<RefCell<View<'static>>>,
) {
    // current view/id
    let v = view.borrow_mut();
    let vid = v.id;

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
    let mut pv = pv.borrow_mut();

    if pv.children.len() != 3 {
        dbg_println!(" pv.children.len({}) != 3", pv.children.len());
        // not handled yet
        return;
    }

    if let Some(ppvid) = pv.parent_id {
        // get grand parent view/id
        let ppv = editor.view_map.get(&ppvid).unwrap().clone();
        let mut ppv = ppv.borrow_mut();

        let pv_layout_index = pv.layout_index.unwrap();

        let mut kept_vid = None;

        // TODO: get sibling ids
        // mark siblings for delete
        for (idx, view_id) in pv.children.iter().enumerate() {
            if idx == v_layout_index {
                dbg_println!("prepare delete of view id {}", *view_id);
                destroy.push(*view_id);
            } else if idx == 1 {
                // separator index
                // TODO: add view_kind ? text/scrollbar/hsplit/vsplit etc ?
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
            // update grand parent focus: // TODO: find a better way
            ppv.focus_to = Some(kept_vid);

            // update link to grand-parent  (new parent)
            let kept_v = editor.view_map.get(&kept_vid).unwrap().clone();
            let mut kept_v = kept_v.borrow_mut();
            kept_v.parent_id = Some(ppvid);
            kept_v.layout_index = Some(pv_layout_index);

            kept_v.destroyable = pv.destroyable; // NB: take parent policy

            dbg_println!("prepare delete of view id {} (parent)", pvid);
            dbg_println!("set focus to view id {}", kept_vid);
            destroy.push(pvid);
            env.focus_changed_to = Some(kept_vid); // post input
        }
    } else {
        // TODO: get sibling ids
        // mark self+siblings for delete
        let mut kept_vid = None;

        for (idx, view_id) in pv.children.iter().enumerate() {
            if idx == v_layout_index {
                dbg_println!("prepare delete of view id {}", *view_id);
                destroy.push(*view_id);
            } else if idx == 1 {
                // separator index
                // TODO: add view_kind ? text/scrollbar/hsplit/vsplit etc ?
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
            let mut kept_v = kept_v.borrow_mut();
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
