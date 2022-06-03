use std::any::Any;
use std::env;
use std::fs;
use std::path::PathBuf;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use crate::core::document::DocumentBuilder;
use crate::core::editor::get_view_by_id;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_view_id;

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

use crate::core::modes::text_mode::center_around_mark;
use crate::core::modes::text_mode::TextModeContext;

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
     { "in": [{ "key": "Up" } ],        "action": "open-doc:select-prev-completion" },
     { "in": [{ "key": "Down" } ],      "action": "open-doc:select-next-completion" },
     { "in": [{ "key": "Right" } ],     "action": "open-doc:apply-current-completion" },
     { "in": [{ "key": "Left" } ],      "action": "open-doc:discard-prompt-suffix" },
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

#[derive(Debug, Clone)]
pub struct OpenDocModeContext {
    pub revision: usize,
    pub controller_view_id: view::Id,
    pub open_doc_completion_view_id: view::Id,
    pub active: bool,
    pub open_doc_str: Vec<char>,
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
            open_doc_completion_view_id: view::Id(0),
            active: false,
            open_doc_str: Vec::new(),
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
        let status_view_id = view::get_status_view(&editor, &env, view);
        if status_view_id.is_none() {
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
            controller.write().parent_id = Some(status_view_id.unwrap());

            v.controller = Some(ControllerView {
                id: odm.controller_view_id,
                mode_name: &"open-doc-mode",
            });

            id
        };

        open_doc_show_controller_view(editor, env, view);
        set_focus_on_view_id(editor, env, controller_id);
    }
}

pub fn open_doc_controller_stop(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let status_view_id = env.status_view_id.unwrap();
        let mut status_view = editor.view_map.get(&status_view_id).unwrap().write();
        status_view.layout_direction = LayoutDirection::Horizontal;
        status_view.children.pop(); // discard child
    }
    {
        let root_view_id = env.root_view_id;
        let mut root_view = editor.view_map.get(&root_view_id).unwrap().write();
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
        set_focus_on_view_id(editor, env, text_view_id);
    }
}

fn create_open_doc_controller_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &mut View,
) {
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

    let mut odm = view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

    // save controller id
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

        register_input_stage_action(
            &mut action_map,
            "open-doc:select-next-completion",
            open_doc_controller_select_next_completion,
        );
        register_input_stage_action(
            &mut action_map,
            "open-doc:select-prev-completion",
            open_doc_controller_select_prev_completion,
        );
        register_input_stage_action(
            &mut action_map,
            "open-doc:apply-current-completion",
            open_doc_controller_apply_current_completion,
        );

        controller_view.register_action_map(action_map);
    }

    editor.add_view(controller_view.id, controller_view);
}

fn open_doc_show_controller_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    text_view: &Rc<RwLock<View<'static>>>,
) {
    let ctrl_view_id = {
        let status_view_id = env.status_view_id.unwrap();

        let mut status_view = editor.view_map.get(&status_view_id).unwrap().write();
        status_view.layout_direction = LayoutDirection::Horizontal;

        let mut text_view = text_view.write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        let ctrl_view_id = odm.controller_view_id;
        status_view.children.pop(); // replace previous child
        status_view.children.push(ChildView {
            id: ctrl_view_id,
            layout_op: LayoutOperation::Percent { p: 100.0 },
        });

        ctrl_view_id
    };

    //
    let controller_view = get_view_by_id(editor, ctrl_view_id);
    let mut controller_view = controller_view.write();

    let mut text_view = text_view.write();
    open_doc_display_path(editor, env, &mut controller_view, &mut text_view);
}

fn open_doc_display_path(
    _editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
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

    dbg_println!("open_doc_display_path end");
}

fn create_open_doc_completion_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    text_view: &mut View,
) {
    let parent_id = env.root_view_id;

    dbg_print!("create_open_doc_completion_view");

    let command_doc = DocumentBuilder::new()
        .document_name("completion-pop-up")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    let modes = vec!["text-mode".to_owned()]; // todo: menu-list
    let mut popup_view = View::new(
        &mut editor,
        &mut env,
        Some(parent_id),
        (0, 0),
        (1, 1),
        command_doc,
        &modes,
        0,
    );
    popup_view.ignore_focus = true;

    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
    odm.open_doc_completion_view_id = popup_view.id;

    editor.add_view(popup_view.id, popup_view);
}

pub fn open_doc_controller_add_char(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
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
    let text_view_view_id = controller_view.controlled_view.unwrap();
    let text_view = editor.view_map.get(&text_view_view_id).unwrap().clone();
    let mut text_view = text_view.write();
    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
    odm.open_doc_str.append(&mut array);
    odm.completion_index = 0;
    {
        let completion_view = get_view_by_id(editor, odm.open_doc_completion_view_id);
        {
            let mut completion_view = completion_view.write();
            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;
        }
        center_around_mark(editor, env, &completion_view);
    }

    dbg_println!("open file : {:?}", odm.open_doc_str);

    open_doc_display_path(editor, env, &mut controller_view, &mut text_view);
}

pub fn open_doc_controller_del_char(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let open_doc_completion_view_id = {
        let v = view.read();
        let text_view_view_id = v.controlled_view.unwrap();
        let mut text_view = editor.view_map.get(&text_view_view_id).unwrap().write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
        if odm.open_doc_str.is_empty() {
            return;
        }
        odm.completion_index = 0;
        odm.open_doc_str.pop();
        odm.open_doc_completion_view_id
    };

    let mut controller_view = view.write();
    let text_view_view_id = controller_view.controlled_view.unwrap();
    let text_view = editor.view_map.get(&text_view_view_id).unwrap().clone();
    let mut text_view = text_view.write();

    {
        let completion_view = get_view_by_id(editor, open_doc_completion_view_id);
        {
            let mut completion_view = completion_view.write();

            // inc
            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;
        }
        center_around_mark(editor, env, &completion_view);
    }

    open_doc_display_path(editor, env, &mut controller_view, &mut text_view);
}

pub fn open_doc_do_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let show = {
        let v = view.read();
        let text_view_view_id = v.controlled_view.unwrap();
        let mut text_view = editor.view_map.get(&text_view_view_id).unwrap().write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        dbg_println!("open file : do completion");

        let s: String = odm.open_doc_str.iter().collect();
        let path = PathBuf::from(s.clone());
        dbg_println!("open file : current directory is '{}'", path.display());

        // path.exist ?
        // if dir and no / at end push '/'
        odm.completion_list.clear();
        odm.completion_index = 0; // if no changes nothing refresh base on meta ?
        match fs::read_dir(path) {
            Ok(path) => {
                for e in path {
                    dbg_println!("open file: dir entry : '{:?}'", e);
                    let s = format!("{}\n", e.unwrap().path().display());
                    dbg_println!("append string '{}'", s);
                    odm.completion_list.push(s.clone());
                }
            }
            _ => {
                /* wrong/incomplete */
                let s = format!("cannot read {}\n", s);
                odm.completion_list.push(s.clone());
                dbg_println!("open file: cannot complete {:?}", s);
            }
        }

        odm.completion_list.sort(); // list.sort_unstable_by(|a, b| (b.0).cmp(&a.0));

        !odm.completion_list.is_empty()
    };
    dbg_println!("show = {}", show);
    if show {
        if let Some(id) = show_completion_popup(editor, env, view) {
            set_focus_on_view_id(editor, env, id);
        }
    } else {
        // hide_completion_popup(editor, env, view);
    }
}

fn show_completion_popup(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    controller_view: &Rc<RwLock<View<'static>>>,
) -> Option<view::Id> {
    // fill completion buffer
    let text_view_id = controller_view.read().controlled_view.unwrap();
    let text_view = get_view_by_id(editor, text_view_id);
    let text_view = text_view.read();
    let odm = text_view.mode_ctx::<OpenDocModeContext>("open-doc-mode");

    let completion_view = get_view_by_id(editor, odm.open_doc_completion_view_id);
    let mut completion_view = completion_view.write();

    let list = &odm.completion_list;
    let doc = completion_view.document().unwrap();
    let mut doc = doc.write();
    doc.delete_content(None);

    for s in list {
        doc.append(s.as_bytes());
    }

    // update position size
    let (st_gx, st_gy, st_w, st_h) = {
        let text_view_view_id = controller_view.read().controlled_view.unwrap();
        let text_view = get_view_by_id(editor, text_view_view_id);

        let status_view_id = view::get_status_view(&editor, &env, &text_view).unwrap();
        let status_view = editor.view_map.get(&status_view_id).unwrap().read();
        (
            status_view.global_x.unwrap(),
            status_view.global_y.unwrap(),
            status_view.width,
            status_view.height,
        )
    };

    // TODO: get view global coordinates, update on  resize
    let parent_id = env.root_view_id;
    let (x, y, pop_width, pop_height) = {
        let parent_view = editor.view_map.get(&parent_id).unwrap().read();
        let dim = parent_view.dimension();
        let w = st_w;
        let h = std::cmp::min(list.len() + 1, dim.1 / 2);
        let x = st_gx;
        let y = st_gy.saturating_sub(h);
        (x, y, w, h)
    };

    completion_view.x = x;
    completion_view.y = y;
    completion_view.width = pop_width;
    completion_view.height = pop_height;

    let mut p_view = editor.view_map.get(&parent_id).unwrap().write();
    p_view.floating_children.pop();
    if p_view.floating_children.is_empty() {
        p_view.floating_children.push(ChildView {
            id: completion_view.id,
            layout_op: LayoutOperation::Floating,
        })
    }

    Some(completion_view.id)
}

// TODO: move mode context to Arc ?
pub fn open_doc_controller_select_next_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.read();
    let text_view_view_id = v.controlled_view.unwrap();
    let text_view = get_view_by_id(editor, text_view_view_id);
    let mut text_view = text_view.write();

    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

    let completion_view = get_view_by_id(editor, odm.open_doc_completion_view_id);
    {
        let mut completion_view = completion_view.write();

        // inc
        odm.completion_index = std::cmp::min(
            odm.completion_index + 1,
            odm.completion_list.len().saturating_sub(1),
        );

        let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");

        let mut offset = 0;
        for i in 0..odm.completion_index {
            let s = &odm.completion_list[i];
            offset += s.len();
        }
        tm.marks[0].offset = offset as u64;
    }

    center_around_mark(editor, env, &completion_view);
}

pub fn open_doc_controller_select_prev_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.read();
    let text_view_view_id = v.controlled_view.unwrap();
    let text_view = get_view_by_id(editor, text_view_view_id);
    let mut text_view = text_view.write();

    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

    let completion_view = get_view_by_id(editor, odm.open_doc_completion_view_id);
    {
        let mut completion_view = completion_view.write();

        // dec
        odm.completion_index = odm.completion_index.saturating_sub(1);
        let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");

        let mut offset = 0;
        for i in 0..odm.completion_index {
            let s = &odm.completion_list[i];
            offset += s.len();
        }

        tm.marks[0].offset = offset as u64;
    }

    center_around_mark(editor, env, &completion_view);
}

pub fn open_doc_controller_apply_current_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
}