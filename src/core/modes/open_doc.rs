use std::any::Any;
use std::env;
use std::fs;
use std::path::PathBuf;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;

use crate::core::view;
use crate::core::view::View;

use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::view::ControllerView;

static OPEN_DOC_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+o" } ],    "action": "open-doc:start" }
    ]
  }
]"#;

static OPEN_DOC_INTERACTIVE_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "open-doc:stop" },
     { "in": [{ "key": "\n" } ],        "action": "open-doc:stop" },
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
        _editor: &mut Editor<'static>,
        _env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        dbg_println!("configure find  {:?}", view.id);

        // setup input map for core actions
        let input_map = build_input_event_map(OPEN_DOC_TRIGGER_MAP).unwrap();
        let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push((self.name(), input_map));
    }

    fn on_view_event(
        &self,
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        _src: ViewEventSource,
        _dst: ViewEventDestination,
        event: &ViewEvent,
        src_view: &mut View<'static>,
        _parent: Option<&mut View<'static>>,
    ) {
        if env.status_view_id.is_none() {
            dbg_println!("open-doc-mode env.status_view_id.is_none()");
            return;
        }

        let src_view_id = src_view.id;
        let svid = env.status_view_id.clone().unwrap();
        dbg_println!("open-doc-mode svid = {:?}", svid);
        match event {
            &ViewEvent::ViewDeselected => {
                let mut gtm = src_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
                dbg_println!("open-doc-mode gtm.active = {}", gtm.active);
                if gtm.active {
                    let status_view = editor.view_map.get(&svid).unwrap();
                    let mut status_view = status_view.write();
                    match &mut status_view.controller {
                        Some(ControllerView { id, mode_name }) => {
                            if *id == src_view_id && *mode_name == "open-doc-mode" {
                                clear_status_view(&mut status_view, &mut gtm);
                            }
                        }

                        _ => {}
                    }
                }
            }

            &ViewEvent::ViewSelected => {
                let src_view_id = src_view.id;
                let mut gtm = src_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
                dbg_println!("open-doc-mode gtm.active = {}", gtm.active);
                if gtm.active {
                    let status_view = editor.view_map.get(&svid).unwrap();
                    let mut status_view = status_view.write();

                    status_view.controller = Some(view::ControllerView {
                        id: src_view_id,
                        mode_name: &"open-doc-mode",
                    });
                    update_status_view(&mut status_view, &mut gtm);
                }
            }

            _ => {}
        }
    }
}

pub struct OpenDocModeContext {
    pub active: bool,
    pub open_doc_str: Vec<char>,

    pub current_dir: String,
    pub current_entry: String,
}

impl OpenDocModeContext {
    pub fn new() -> Self {
        dbg_println!("OpenDocModeContext");
        OpenDocModeContext {
            active: false,
            open_doc_str: Vec::new(),
            current_dir: String::new(),
            current_entry: String::new(),
        }
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
        register_input_stage_action(&mut map, "open-doc:stop", open_doc_stop);
        register_input_stage_action(&mut map, "open-doc:add-char", open_doc_add_char);
        register_input_stage_action(&mut map, "open-doc:del-char", open_doc_del_char);
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

        let svid = status_vid.unwrap();

        let status_view = editor.view_map.get(&svid).unwrap();

        // start/resume ?
        let already_active = {
            let mut v = view.write();
            let gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
            let already_active = gtm.active;
            gtm.active = true;

            status_view.write().controller = Some(view::ControllerView {
                id: v.id,
                mode_name: &"open-doc-mode",
            });
            already_active
        };

        //
        let doc = status_view.read().document().unwrap();
        let mut doc = doc.write();

        // clear status view
        doc.delete_content(None);

        // setup working directory
        {
            let mut v = view.write();
            let gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

            if gtm.open_doc_str.is_empty() {
                let path = env::current_dir().unwrap();
                let s = path.to_str().unwrap();
                let s = s.to_owned();
                for c in s.chars() {
                    gtm.open_doc_str.push(c);
                }
                gtm.open_doc_str.push('/');
            }
        }

        if !already_active {
            // setup new input map
            let mut v = view.write();
            v.input_ctx.stack_pos = None;
            let input_map = build_input_event_map(OPEN_DOC_INTERACTIVE_MAP).unwrap();
            let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(("open-doc-mode", input_map));
        }
    }

    display_open_doc_string(editor, env, &view);
}

pub fn open_doc_stop(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let v = view.write();
        let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.pop();
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

        {
            let mut v = view.write();
            let gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

            // gtm.reset();
            gtm.open_doc_str.clear();
            gtm.active = false;
        }
    }
}

pub fn open_doc_do_completion(
    _editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut v = view.write();
    let gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

    dbg_println!("open file : do completion");

    let s: String = gtm.open_doc_str.iter().collect();
    let path = PathBuf::from(s);
    dbg_println!("open file : current directory is '{}'", path.display());

    // path.exist ?
    // if dir and no / at end push '/'

    match fs::read_dir(path) {
        Ok(path) => {
            for e in path {
                dbg_println!("open file: dir entry : '{}'", e.unwrap().path().display())
            }
        }
        _ => {
            /* wrong/incomplete */
            dbg_println!("open file: cannot complete");
        }
    }
}

pub fn open_doc_add_char(
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
    } else {
        let mut v = view.write();
        let gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
        gtm.open_doc_str.append(&mut array);
        dbg_println!("open file : {:?}", gtm.open_doc_str);
    }

    display_open_doc_string(&mut editor, &mut env, &view);
}

pub fn open_doc_del_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let mut v = view.write();
        let gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
        if gtm.open_doc_str.is_empty() {
            return;
        }
        gtm.open_doc_str.pop();
    }

    display_open_doc_string(&mut editor, &mut env, &view);
}

fn clear_status_view(status_view: &mut View, _fm: &mut OpenDocModeContext) {
    // clear status
    let doc = status_view.document().unwrap();
    let mut doc = doc.write();
    // clear buffer. doc.erase_all();
    doc.delete_content(None);
}

pub fn display_open_doc_string(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    //
    // reset status view : TODO(ceg): view::reset_status_view(&editor, view);
    let status_vid = view::get_status_view(&editor, &env, view);

    if let Some(status_vid) = status_vid {
        let mut v = view.write();
        let mut gtm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        let mut status_view = editor.view_map.get(&status_vid).unwrap().write();
        update_status_view(&mut status_view, &mut gtm);
    }
}

fn update_status_view(status_view: &mut View, gtm: &mut OpenDocModeContext) {
    let doc = status_view.document().unwrap();
    let mut doc = doc.write();

    doc.delete_content(None);
    doc.append("Open: ".as_bytes());

    if !gtm.open_doc_str.is_empty() {
        let s: String = gtm.open_doc_str.iter().collect();
        let d = &s.as_bytes();
        doc.append(d);
    }
}
