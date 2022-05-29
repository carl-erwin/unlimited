use std::any::Any;
use std::env;
use std::fs;
use std::path::PathBuf;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use super::text_mode::TextModeContext;

use super::text_mode::PostInputAction;

use crate::core::document::get_document_byte_count;

use crate::core::document::DocumentBuilder;
use crate::core::editor::get_view_by_id;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_vid;

use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;

use crate::core::view;
use crate::core::view::ChildView;
use crate::core::view::View;

use crate::core::view::ControllerView;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;

use crate::core::editor;

static OPEN_DOC_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+o" } ],    "action": "open-doc:start" }
    ]
  }
]"#;

static OPEN_DOC_CONTROLLER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "open-doc:stop" },
     { "in": [{ "key": "\n" } ],        "action": "open-doc:stop" },
     { "in": [{ "key": "ctrl+g" } ],    "action": "open-doc:stop" },
     { "in": [{ "key": "BackSpace" } ], "action": "open-doc:del-char" },
     { "in": [{ "key": "Delete" } ],    "action": "open-doc:do-nothing" },
     { "default": [],                   "action": "open-doc:add-char" }
   ]
  }

]"#;

impl<'a> Mode for OpenDocMode {
    fn name(&self) -> &'static str {
        &"open-doc-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc open-doc-mode ctx");
        let ctx = OpenDocModeContext::new();
        Box::new(ctx)
    }

    fn configure_view(
        &mut self,
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        dbg_println!("configure find  {:?}", view.id);

        // setup input map for core actions
        {
            let input_map = build_input_event_map(OPEN_DOC_TRIGGER_MAP).unwrap();
            let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push((self.name(), input_map));
        }

        // add controller view
        create_open_doc_controller_view(editor, env, view);

        // add completion view
        create_open_doc_completion_view(editor, env, view);
    }
}

pub struct OpenDocModeContext {
    pub revision: usize,
    pub controller_view_id: view::Id,
    pub open_doc_completion_vid: view::Id,
    pub active: bool,
    pub open_doc_str: Vec<char>,
    pub completion_str: String,
    pub current_dir: String,
    pub current_entry: String,
    pub completion_list: Vec<String>,
    pub completion_index: usize,
}

impl OpenDocModeContext {
    pub fn new() -> Self {
        dbg_println!("OpenDocModeContext");
        OpenDocModeContext {
            revision: 0,
            controller_view_id: view::Id(0),
            open_doc_completion_vid: view::Id(0),
            active: false,
            open_doc_str: Vec::new(),
            completion_str: String::new(),
            current_dir: String::new(),
            current_entry: String::new(),
            completion_list: vec![],
            completion_index: 0,
        }
    }
    pub fn reset(&mut self) -> &mut Self {
        self.revision = 0;
        self.active = false;
        self.open_doc_str.clear();
        self.completion_str.clear();
        self.current_dir.clear();
        self.current_entry.clear();
        self.completion_list = vec![];
        self.completion_index = 0;

        self
    }
}
pub struct OpenDocMode {
    // add common fields
}

impl OpenDocMode {
    pub fn new() -> Self {
        dbg_println!("OpenDocMode");
        OpenDocMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "open-doc:start", open_doc_start);
    }
}

pub fn open_doc_start(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let status_vid = view::get_status_view(&editor, &env, view);
        if status_vid.is_none() {
            // TODO(ceg): log missing status mode
            return;
        }

        // start/resume ?
        let controller_id = {
            let mut v = view.write();
            let odm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
            odm.active = true;

            let id = odm.controller_view_id;

            // attach to status view
            let controller = editor.view_map.get(&id).unwrap();
            controller.write().parent_id = Some(status_vid.unwrap());

            v.controller = Some(ControllerView {
                id: odm.controller_view_id,
                mode_name: &"open-doc-mode",
            });

            id
        };

        open_doc_show_controller_view(editor, env, view);
        set_focus_on_vid(editor, env, controller_id);
    }
}

pub fn open_doc_controller_stop(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let status_vid = env.status_view_id.unwrap();
        let mut status_view = editor.view_map.get(&status_vid).unwrap().write();
        status_view.layout_direction = LayoutDirection::Horizontal;
        status_view.children.pop(); // discard child
    }
    {
        let root_vid = env.root_view_id;
        let mut root_view = editor.view_map.get(&root_vid).unwrap().write();
        root_view.floating_children.pop(); // discard child
    }

    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        {
            let mut text_view = editor.view_map.get(&text_view_id).unwrap().write();
            text_view.controller = None;

            let otm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
            otm.reset();

            //
            let doc = v.document().unwrap();
            let mut doc = doc.write();
            doc.delete_content(None);
        }

        // set input focus to
        set_focus_on_vid(editor, env, text_view_id);
    }
}

fn create_open_doc_controller_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &mut View,
) {
    // get status vid -> status_vid

    // (w,h) = status_vid.dimension()
    let (x, y) = (0, 0);
    let (w, h) = (1, 1);

    let doc = DocumentBuilder::new()
        .document_name("goto-controller")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    // create view at mode creation
    let mut controller_view = View::new(
        &mut editor,
        &mut env,
        None,
        (x, y),
        (w, h),
        doc,
        &vec!["status-mode".to_owned()], // TODO(ceg): goto-line-controller
        0,
    );

    controller_view.ignore_focus = false;

    controller_view.controlled_view = Some(view.id);

    // set controller target as view.id
    let mut odm = view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

    odm.controller_view_id = controller_view.id;

    dbg_println!("odm.controller_view_id = {:?}", odm.controller_view_id);

    // setup new input map
    {
        controller_view.input_ctx.stack_pos = None;

        {
            let event_map = build_input_event_map(OPEN_DOC_CONTROLLER_MAP).unwrap();
            let mut input_map_stack = controller_view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(("open-doc-controller", event_map));
        }

        let mut action_map = InputStageActionMap::new();

        register_input_stage_action(&mut action_map, "open-doc:stop", open_doc_controller_stop);
        register_input_stage_action(
            &mut action_map,
            "open-doc:add-char",
            open_doc_controller_add_char,
        );
        register_input_stage_action(
            &mut action_map,
            "open-doc:del-char",
            open_doc_controller_del_char,
        );

        controller_view.register_action_map(action_map);
    }

    editor.add_view(controller_view.id, controller_view);
}

fn open_doc_show_controller_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    text_view: &Rc<RwLock<View<'static>>>,
) {
    let ctrl_vid = {
        let status_vid = env.status_view_id.unwrap();

        let mut status_view = editor.view_map.get(&status_vid).unwrap().write();
        status_view.layout_direction = LayoutDirection::Horizontal;

        let mut text_view = text_view.write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        let ctrl_vid = odm.controller_view_id;
        status_view.children.pop(); // replace previous child
        status_view.children.push(ChildView {
            id: ctrl_vid,
            layout_op: LayoutOperation::Percent { p: 100.0 },
        });

        ctrl_vid
    };

    //
    let controller_view = get_view_by_id(editor, ctrl_vid);
    let mut controller_view = controller_view.write();

    let mut text_view = text_view.write();
    open_doc_display_path(editor, env, &mut controller_view, &mut text_view);
}

fn open_doc_display_path(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    controller_view: &mut View<'static>,
    text_view: &mut View<'static>,
) {
    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
    let doc = controller_view.document().clone();
    let mut doc = doc.as_ref().unwrap().write();

    doc.delete_content(None);
    doc.append("Open: ".as_bytes());

    // setup working directory
    {
        if odm.open_doc_str.is_empty() {
            let path = env::current_dir().unwrap();
            let s = path.to_str().unwrap();
            let s = s.to_owned();
            for c in s.chars() {
                odm.open_doc_str.push(c);
            }
            odm.open_doc_str.push('/');
        }

        let s: String = odm.open_doc_str.iter().collect();
        doc.append(s.as_bytes());
    }
}

fn create_open_doc_completion_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &mut View,
) {
}

pub fn open_doc_controller_add_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut array = vec![];

    // filter input event
    let mut do_completion = false;
    {
        let v = view.read();

        assert!(v.input_ctx.trigger.len() > 0);
        let idx = v.input_ctx.trigger.len() - 1;

        dbg_println!("open file : env {:?}", v.input_ctx.trigger[idx]);

        match &v.input_ctx.trigger[idx] {
            InputEvent::KeyPress {
                key: Key::Unicode(c),
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
            } => {
                if *c == '\t' {
                    do_completion = true;
                } else {
                    array.push(*c);
                }
            }

            _ => {
                return;
            }
        }

        if array.is_empty() && !do_completion {
            return;
        }
    }

    if do_completion {
        open_doc_do_completion(editor, env, view);
    }

    let mut controller_view = view.write();
    let text_view_vid = controller_view.controlled_view.unwrap();
    let text_view = editor.view_map.get(&text_view_vid).unwrap().clone();
    let mut text_view = text_view.write();
    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
    odm.open_doc_str.append(&mut array);
    dbg_println!("open file : {:?}", odm.open_doc_str);

    open_doc_display_path(editor, env, &mut controller_view, &mut text_view);
}

pub fn open_doc_controller_del_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let v = view.read();
        let text_view_vid = v.controlled_view.unwrap();
        let mut text_view = editor.view_map.get(&text_view_vid).unwrap().write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
        if odm.open_doc_str.is_empty() {
            return;
        }
        odm.open_doc_str.pop();
    }

    let mut controller_view = view.write();
    let text_view_vid = controller_view.controlled_view.unwrap();
    let text_view = editor.view_map.get(&text_view_vid).unwrap().clone();
    let mut text_view = text_view.write();

    open_doc_display_path(editor, env, &mut controller_view, &mut text_view);
}

pub fn open_doc_do_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let show = {
        let v = view.read();
        let text_view_vid = v.controlled_view.unwrap();
        let mut text_view = editor.view_map.get(&text_view_vid).unwrap().write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        dbg_println!("open file : do completion");

        let s: String = odm.open_doc_str.iter().collect();
        let path = PathBuf::from(s.clone());
        dbg_println!("open file : current directory is '{}'", path.display());

        // path.exist ?
        // if dir and no / at end push '/'
        odm.completion_str.clear();
        match fs::read_dir(path) {
            Ok(path) => {
                for e in path {
                    let s = format!("{}\n", e.unwrap().path().display());
                    odm.completion_list.push(s.clone());
                    odm.completion_str.push_str(&s);
                    dbg_println!("open file: dir entry : '{}'", s);
                }
            }
            _ => {
                /* wrong/incomplete */
                let s = format!("cannot read {}\n", s);
                odm.completion_str.push_str(&s);

                dbg_println!("open file: cannot complete {:?}", s);
            }
        }

        !odm.completion_str.is_empty()
    };
    dbg_println!("show = {}", show);
    if show {
        show_completion_popup(editor, env, view);

        // set input focus to
        // destroy previous popup
        let id = {
            let parent_id = env.root_view_id;
            let p_view = editor.view_map.get(&parent_id).unwrap().read();
            p_view.floating_children[0].id
        };

        set_focus_on_vid(editor, env, id);
    } else {
        // hide_completion_popup(editor, env, view);
    }
}

fn show_completion_popup(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    control_view: &Rc<RwLock<View<'static>>>,
) {
    let parent_id = env.root_view_id;

    let command_doc = DocumentBuilder::new()
        .document_name("completion-pop-up")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    {
        let v = control_view.read();
        let text_view_vid = v.controlled_view.unwrap();
        let text_view = editor.view_map.get(&text_view_vid).unwrap().read();
        let list = text_view
            .mode_ctx::<OpenDocModeContext>("open-doc-mode")
            .completion_list
            .clone();

        let mut d = command_doc.as_ref().unwrap().write();

        for s in &list {
            d.append(s.as_bytes());
        }
    }

    let (st_gx, st_gy, st_w, st_h) = {
        let text_view_vid = control_view.read().controlled_view.unwrap();
        let text_view = get_view_by_id(editor, text_view_vid);
        let status_vid = view::get_status_view(&editor, &env, &text_view);
        if let Some(status_vid) = status_vid {
            let status_view = editor.view_map.get(&status_vid).unwrap().read();
            (
                status_view.global_x.unwrap(),
                status_view.global_y.unwrap(),
                status_view.width,
                status_view.height,
            )
        } else {
            return;
        }
    };

    // TODO: get view global coordinates, update on  resize
    let (x, y, pop_width, pop_height) = {
        let parent_view = editor.view_map.get(&parent_id).unwrap().read();
        let dim = parent_view.dimension();

        let w = dim.0;
        let h = std::cmp::min(8, dim.1 / 2);
        let x = st_gx;
        let y = st_gy.saturating_sub(h);
        (x, y, w, h)
    };

    ////////////////////////////
    let modes = vec!["text-mode".to_owned()];

    // create view
    let mut popup_view = View::new(
        &mut editor,
        &mut env,
        Some(parent_id),
        (x, y),
        (pop_width, pop_height),
        command_doc,
        &modes,
        0,
    );

    popup_view.ignore_focus = true;

    {
        let mut parent_view = editor.view_map.get(&parent_id).unwrap().write();
        parent_view.floating_children.push(ChildView {
            id: popup_view.id,
            layout_op: LayoutOperation::Floating,
        });
    }

    editor.add_view(popup_view.id, popup_view);
}
