use std::any::Any;
use std::env;
use std::fs;
use std::path::PathBuf;

use parking_lot::RwLock;

use std::rc::Rc;
use std::sync::Arc;

use super::Mode;

use crate::core::buffer::BufferBuilder;
use crate::core::buffer::BufferKind;
use crate::core::editor::get_view_by_id;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_view_id;

use crate::core::path_to_buffer_kind;

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
use crate::core::view::LayoutSize;

use crate::core::modes::text_mode::center_around_mark_if_offscreen;
use crate::core::modes::text_mode::TextModeContext;

use crate::core::build_view_layout_typed;
use crate::core::parse_layout_str;
use crate::core::DEFAULT_LAYOUT_JSON;

static OPEN_DOC_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+o" } ],   "action": "open-doc:start" }
     ]
  }
]"#;

static OPEN_DOC_CONTROLLER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "open-doc:stop" },
     { "in": [{ "key": "\n" } ],        "action": "open-doc:show-buffer" },
     { "in": [{ "key": "ctrl+q" } ],    "action": "open-doc:stop" },
     { "in": [{ "key": "BackSpace" } ], "action": "open-doc:del-char" },
     { "in": [{ "key": "Delete" } ],    "action": "open-doc:do-nothing" },
     { "in": [{ "key": "Up" } ],        "action": "open-doc:select-prev-completion" },
     { "in": [{ "key": "alt+i" } ],    "action": "open-doc:select-prev-completion" },
     { "in": [{ "key": "Down" } ],      "action": "open-doc:select-next-completion" },
     { "in": [{ "key": "alt+k" } ],    "action": "open-doc:select-next-completion" },
     { "in": [{ "key": "Left" } ],      "action": "open-doc:discard-prompt-suffix" },
     { "in": [{ "key": "alt+j" } ],    "action": "open-doc:discard-prompt-suffix" },
     { "in": [{ "key": "Right" } ],     "action": "open-doc:apply-current-completion" },
     { "in": [{ "key": "ctrl+Space" } ],"action": "open-doc:apply-current-completion" },
     { "in": [{ "key": "ctrl+Enter" } ],"action": "open-doc:apply-current-completion" },
     { "in": [{ "key": "alt+l" } ],    "action": "open-doc:apply-current-completion" },
     { "in": [{ "key": "Home" } ],      "action": "open-doc:select-first-completion" },
     { "in": [{ "key": "End" } ],       "action": "open-doc:select-last-completion" },
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
        Box::new(OpenDocModeContext::new())
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
    pub completion_view_id: view::Id,
    pub active: bool,
    pub prompt: Vec<char>,
    pub completion_list: Vec<String>,
    pub completion_index: usize,
    pub error_msg: Option<String>,
}

impl OpenDocModeContext {
    pub fn new() -> Self {
        dbg_println!("OpenDocModeContext");
        OpenDocModeContext {
            revision: 0,
            controller_view_id: view::Id(0),
            completion_view_id: view::Id(0),
            active: false,
            prompt: Vec::new(),
            completion_list: vec![],
            completion_index: 0,
            error_msg: None,
        }
    }
    pub fn reset(&mut self) -> &mut Self {
        self.revision = 0;
        self.active = false;
        self.prompt.clear();
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
        let status_view_id = view::get_command_view_id(editor, env);
        if status_view_id.is_none() {
            // TODO(ceg): log missing status mode
            dbg_println!("status view is missing");
            return;
        }

        // start/resume ?

        let controller_view_id = {
            let mut v = view.write();
            let odm = v.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
            odm.active = true;

            let controller_view_id = odm.controller_view_id;

            // attach to status view
            let controller = get_view_by_id(editor, controller_view_id);
            controller.write().parent_id = Some(status_view_id.unwrap());

            v.controller = Some(ControllerView {
                id: odm.controller_view_id,
                mode_name: &"open-doc-mode",
            });

            controller_view_id
        };

        open_doc_show_controller_view(editor, env, view);
        set_focus_on_view_id(editor, env, controller_view_id);

        {
            let controller_view = get_view_by_id(editor, controller_view_id);
            open_doc_do_completion(editor, env, &controller_view, false);
        }
    }
}

pub fn open_doc_controller_stop(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let status_view_id = env.status_view_id.unwrap();
        let status_view = get_view_by_id(editor, status_view_id);
        let mut status_view = status_view.write();

        status_view.layout_direction = LayoutDirection::Horizontal;
        status_view.children.pop(); // discard child
    }
    {
        let root_view_id = env.root_view_id;
        get_view_by_id(editor, root_view_id)
            .write()
            .floating_children
            .pop(); // discard child
    }

    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        {
            let text_view = get_view_by_id(editor, text_view_id);
            let mut text_view = text_view.write();
            text_view.controller = None;

            let otm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
            otm.reset();

            //
            let buffer = v.buffer().unwrap();
            let mut buffer = buffer.write();
            buffer.delete_content(None);
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

    let buffer = BufferBuilder::new(BufferKind::File)
        .buffer_name("open-doc-controller")
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
        buffer,
        &vec![],                             // tags
        &vec!["empty-line-mode".to_owned()], // modes: TODO(ceg): -controller
        0,
        LayoutDirection::NotSet,
        LayoutSize::Percent { p: 100.0 },
    );

    controller_view.ignore_focus = false;

    controller_view.controlled_view = Some(view.id);

    let odm = view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

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

        register_input_stage_action(
            &mut action_map,
            "open-doc:show-buffer",
            open_doc_controller_show_buffer,
        );

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

        register_input_stage_action(
            &mut action_map,
            "open-doc:discard-prompt-suffix",
            open_doc_controller_discard_prompt_suffix,
        );

        register_input_stage_action(
            &mut action_map,
            "open-doc:select-first-completion",
            open_doc_controller_select_first_completion,
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

        let status_view = get_view_by_id(editor, status_view_id);
        let mut status_view = status_view.write();

        status_view.layout_direction = LayoutDirection::Horizontal;

        let mut text_view = text_view.write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        let ctrl_view_id = odm.controller_view_id;
        status_view.children.pop(); // replace previous child
        status_view.children.push(ChildView {
            id: ctrl_view_id,
            layout_op: LayoutSize::Percent { p: 100.0 },
        });

        ctrl_view_id
    };
    //
    let controller_view = get_view_by_id(editor, ctrl_view_id);
    let mut controller_view = controller_view.write();
    let mut text_view = text_view.write();
    open_doc_display_prompt(editor, env, &mut controller_view, &mut text_view);
}

fn open_doc_display_prompt(
    _editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    controller_view: &mut View<'static>,
    text_view: &mut View<'static>,
) {
    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
    let buffer = controller_view.buffer().clone();
    let mut buffer = buffer.as_ref().unwrap().write();

    buffer.delete_content(None);
    buffer.append("Open: ".as_bytes());

    // setup working directory
    {
        if odm.prompt.is_empty() {
            let path = env::current_dir().unwrap();
            let s = path.to_str().unwrap();
            let s = s.to_owned();
            for c in s.chars() {
                odm.prompt.push(c);
            }
            odm.prompt.push(std::path::MAIN_SEPARATOR);
        }

        let s: String = odm.prompt.iter().collect();
        buffer.append(s.as_bytes());
    }

    dbg_println!("open_doc_display_prompt end");
}

fn create_open_doc_completion_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    text_view: &mut View,
) {
    let parent_id = env.root_view_id;

    dbg_print!("create_open_doc_completion_view");

    let command_buffer = BufferBuilder::new(BufferKind::File)
        .buffer_name("completion-pop-up")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    let tags = vec![]; // todo: menu-list
    let modes = vec!["text-mode".to_owned()]; // todo: menu-list
    let mut popup_view = View::new(
        &mut editor,
        &mut env,
        Some(parent_id),
        (0, 0),
        (1, 1),
        command_buffer,
        &tags,
        &modes,
        0,
        LayoutDirection::NotSet,
        LayoutSize::Percent { p: 100.0 },
    );
    popup_view.ignore_focus = true;

    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
    odm.completion_view_id = popup_view.id;

    editor.add_view(popup_view.id, popup_view);
}

pub fn open_doc_controller_add_char(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut array = vec![];

    let mut auto_complete = false;

    // filter input event
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
                // do not allow tabs ?

                if *c == '\n' {
                    panic!();
                }

                if *c != '\t' {
                    array.push(*c);
                } else {
                    auto_complete = true;
                    // TODO: if all with same suffix append suffix
                }
            }

            _ => {
                return;
            }
        }

        if array.is_empty() && !auto_complete {
            return;
        }
    }

    let completion_view_id = {
        let v = view.read();
        let text_view_view_id = v.controlled_view.unwrap();

        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();

        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
        odm.prompt.append(&mut array);
        odm.completion_index = 0;
        odm.completion_view_id
    };

    open_doc_do_completion(editor, env, view, auto_complete);

    let mut controller_view = view.write();
    let text_view_view_id = controller_view.controlled_view.unwrap();

    let text_view = get_view_by_id(editor, text_view_view_id);
    let mut text_view = text_view.write();
    {
        let completion_view = get_view_by_id(editor, completion_view_id);
        {
            let mut completion_view = completion_view.write();
            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;
        }
        center_around_mark_if_offscreen(editor, env, &completion_view);
    }
    open_doc_display_prompt(editor, env, &mut controller_view, &mut text_view);
}

pub fn open_doc_controller_del_char(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let completion_view_id = {
        let v = view.read();
        let text_view_view_id = v.controlled_view.unwrap();

        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();

        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");
        if odm.prompt.len() <= 1 {
            return;
        }
        odm.completion_index = 0;
        odm.prompt.pop();
        odm.completion_view_id
    };

    {
        open_doc_do_completion(editor, env, view, false);
    }

    let mut controller_view = view.write();
    let text_view_view_id = controller_view.controlled_view.unwrap();
    let text_view = editor
        .view_map
        .read()
        .get(&text_view_view_id)
        .unwrap()
        .clone();
    let mut text_view = text_view.write();

    {
        let completion_view = get_view_by_id(editor, completion_view_id);
        {
            let mut completion_view = completion_view.write();
            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;
        }
        center_around_mark_if_offscreen(editor, env, &completion_view);
    }

    open_doc_display_prompt(editor, env, &mut controller_view, &mut text_view);
}

pub fn open_doc_do_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    auto_complete: bool,
) {
    dbg_println!("open file : do completion");

    {
        let v = view.read();
        let text_view_view_id = v.controlled_view.unwrap();

        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();

        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        // clear completion list/index
        odm.completion_list.clear();
        odm.completion_index = 0;

        let s: String = odm.prompt.iter().collect();
        let (prefix, suffix) = if let Some(last_sep) = s.rfind(std::path::MAIN_SEPARATOR) {
            s.split_at(last_sep + 1)
        } else {
            (s.as_str(), "")
        };

        dbg_println!(
            "open file : do completion: prefix {}, suffix '{}'",
            prefix,
            suffix
        );

        dbg_println!("open file : do completion: prompt '{}'", s);

        let path = PathBuf::from(prefix);
        dbg_println!("do completion: for '{:?}'", path);
        match fs::read_dir(&path) {
            Ok(path) => {
                odm.error_msg = None;

                for e in path {
                    dbg_println!("do completion: parent_path entry : '{:?}'", e);
                    let cur_path = PathBuf::from(e.as_ref().unwrap().path());

                    if let Some(suffix2) = cur_path.iter().last() {
                        let match_suffix = suffix2.to_str().unwrap().starts_with(&suffix);
                        if match_suffix {
                            dbg_println!("do completion: found possible completion '{:?}'", e);

                            let mut s = e.unwrap().path().to_str().unwrap().to_owned();

                            // if dir add std::path::MAIN_SEPARATOR
                            match fs::metadata(&s) {
                                Ok(metadata) => {
                                    if metadata.is_dir() {
                                        s.push(std::path::MAIN_SEPARATOR);
                                    }
                                }
                                _ => {}
                            }

                            s.push('\n');
                            odm.completion_list.push(s);
                        }
                    }
                }
            }

            _ => {
                dbg_println!("open file: cannot read {:?}", s);
                let s = format!("cannot read '{}'\n", s);
                odm.error_msg = Some(s);
                odm.completion_list.clear();
                odm.completion_index = 0;
            }
        }

        // auto complete ?
        if odm.completion_list.len() == 1 && auto_complete {
            let len = odm.completion_list[0].len().saturating_sub(1);

            odm.prompt = odm.completion_list[0]
                .chars()
                .take(len)
                .collect::<Vec<char>>();
        }

        odm.completion_list.sort(); // list.sort_unstable_by(|a, b| (b.0).cmp(&a.0));
        let has_item = !odm.completion_list.is_empty();
        has_item
    };

    if let Some(_id) = show_completion_popup(editor, env, view) {
        // set_focus_on_view_id(editor, env, id);
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

    let completion_view = get_view_by_id(editor, odm.completion_view_id);
    let mut completion_view = completion_view.write();

    let buffer = completion_view.buffer().unwrap();
    let mut buffer = buffer.write();
    buffer.delete_content(None);

    match &odm.error_msg {
        Some(msg) => {
            buffer.append(msg.as_bytes());
        }
        _ => {
            let list = &odm.completion_list;
            for s in list {
                buffer.append(s.as_bytes());
            }
        }
    }

    // update position size
    let (st_gx, st_gy, st_w, _st_h) = {
        let status_view_id = view::get_command_view_id(editor, &env).unwrap();
        let status_view = get_view_by_id(editor, status_view_id);
        let status_view = status_view.read();
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
        let dim = get_view_by_id(editor, parent_id).read().dimension();
        let w = st_w;
        //        let h = std::cmp::min(list.len(), dim.1 / 2);
        let h = dim.1.saturating_sub(_st_h); // / 3 + dim.1 / 3;
        let x = st_gx;
        let y = st_gy.saturating_sub(h);
        (x, y, w, h)
    };

    completion_view.x = x;
    completion_view.y = y;
    completion_view.width = pop_width;
    completion_view.height = pop_height;

    let p_view = get_view_by_id(editor, parent_id);
    let mut p_view = p_view.write();
    p_view.floating_children.pop();
    if p_view.floating_children.is_empty() {
        p_view.floating_children.push(ChildView {
            id: completion_view.id,
            layout_op: LayoutSize::Floating,
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

    if odm.completion_list.is_empty() {
        return;
    }

    if odm.error_msg.is_some() {
        return;
    }

    let completion_view = get_view_by_id(editor, odm.completion_view_id);
    {
        let mut completion_view = completion_view.write();

        // inc
        odm.completion_index = (odm.completion_index + 1) % odm.completion_list.len();

        let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");

        let mut offset = 0;
        for i in 0..odm.completion_index {
            let s = &odm.completion_list[i];
            offset += s.len();
        }
        tm.marks[0].offset = offset as u64;
    }

    center_around_mark_if_offscreen(editor, env, &completion_view);
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

    if odm.error_msg.is_some() {
        return;
    }

    let completion_view = get_view_by_id(editor, odm.completion_view_id);
    {
        let mut completion_view = completion_view.write();

        // dec
        odm.completion_index = if odm.completion_index == 0 {
            odm.completion_list.len()
        } else {
            odm.completion_index
        };
        odm.completion_index = odm.completion_index.saturating_sub(1);

        let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");

        let mut offset = 0;
        for i in 0..odm.completion_index {
            let s = &odm.completion_list[i];
            offset += s.len();
        }

        tm.marks[0].offset = offset as u64;
    }

    center_around_mark_if_offscreen(editor, env, &completion_view);
}

pub fn open_doc_controller_apply_current_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let completion_view_id = {
        let controller_view = view.write();
        let text_view_view_id = controller_view.controlled_view.unwrap();
        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        if odm.completion_list.is_empty() {
            return;
        }

        let completion_view = get_view_by_id(editor, odm.completion_view_id);
        {
            let mut completion_view = completion_view.write();

            {
                let buffer = completion_view.buffer().unwrap();
                let mut buffer = buffer.write();
                buffer.delete_content(None);
            }

            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;

            let s = &odm.completion_list[odm.completion_index];
            let len = s.len().saturating_sub(1); // remove last '\n'
            let new_prompt = s.chars().take(len).collect::<Vec<char>>();

            odm.revision = 0;
            odm.completion_list = vec![];
            odm.completion_index = 0;
            odm.prompt = new_prompt;

            odm.completion_view_id
        }
    };

    open_doc_do_completion(editor, env, view, false);

    {
        let mut controller_view = view.write();
        let text_view_view_id = controller_view.controlled_view.unwrap();
        let text_view = editor
            .view_map
            .read()
            .get(&text_view_view_id)
            .unwrap()
            .clone();
        let mut text_view = text_view.write();

        {
            let completion_view = get_view_by_id(editor, completion_view_id);
            {
                let mut completion_view = completion_view.write();
                let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks[0].offset = 0;
            }
            center_around_mark_if_offscreen(editor, env, &completion_view);
        }
        open_doc_display_prompt(editor, env, &mut controller_view, &mut text_view);
    }
}

pub fn open_doc_controller_discard_prompt_suffix(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let completion_view_id = {
        let controller_view = view.write();
        let text_view_view_id = controller_view.controlled_view.unwrap();
        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        let completion_view = get_view_by_id(editor, odm.completion_view_id);
        {
            let mut completion_view = completion_view.write();

            {
                let buffer = completion_view.buffer().unwrap();
                let mut buffer = buffer.write();
                buffer.delete_content(None);
            }

            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;

            odm.revision = 0;
            odm.completion_list = vec![];
            odm.completion_index = 0;

            // if last character is std::path::MAIN_SEPARATOR, pop twice
            let count = if *odm.prompt.last().unwrap_or(&' ') == std::path::MAIN_SEPARATOR {
                2
            } else {
                1
            };

            for _ in 0..count {
                let s: String = odm.prompt.iter().collect();
                dbg_println!("do completion split prompt: {}", s);

                let (prefix, suffix) = if let Some(last_sep) = s.rfind(std::path::MAIN_SEPARATOR) {
                    s.split_at(last_sep + 1)
                } else {
                    (s.as_str(), "")
                };

                dbg_println!("do completion prefix: {}", prefix);
                dbg_println!("do completion suffix: {}", suffix);

                odm.prompt = prefix.to_owned().chars().collect();
                if suffix.is_empty() && odm.prompt.len() > 1 {
                    odm.prompt.pop();
                }
            }

            odm.completion_view_id
        }
    };

    open_doc_do_completion(editor, env, view, false);

    // factorize this with helper
    {
        let mut controller_view = view.write();
        let text_view_view_id = controller_view.controlled_view.unwrap();
        let text_view = editor
            .view_map
            .read()
            .get(&text_view_view_id)
            .unwrap()
            .clone();
        let mut text_view = text_view.write();

        {
            let completion_view = get_view_by_id(editor, completion_view_id);
            {
                let mut completion_view = completion_view.write();
                let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
                tm.marks[0].offset = 0;
            }
            center_around_mark_if_offscreen(editor, env, &completion_view);
        }
        open_doc_display_prompt(editor, env, &mut controller_view, &mut text_view);
    }
}

pub fn open_doc_controller_select_first_completion(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let controller_view = view.write();
        let text_view_view_id = controller_view.controlled_view.unwrap();
        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();
        let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

        if odm.completion_list.is_empty() {
            return;
        }

        let completion_view = get_view_by_id(editor, odm.completion_view_id);
        {
            let mut completion_view = completion_view.write();
            let tm = completion_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks[0].offset = 0;
            odm.completion_index = 0;
        }
    }

    open_doc_do_completion(editor, env, view, false);

    {
        let mut controller_view = view.write();
        let text_view_view_id = controller_view.controlled_view.unwrap();
        let text_view = get_view_by_id(editor, text_view_view_id);
        let mut text_view = text_view.write();
        open_doc_display_prompt(editor, env, &mut controller_view, &mut text_view);
    }
}

pub fn open_doc_controller_show_buffer(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let (root_view_idx, new_root_view_id, ok) = open_doc_controller_load_buffer(editor, env, view);
    if !ok {
        return;
    }

    open_doc_controller_stop(editor, env, view);

    // switch
    env.root_view_index = root_view_idx;
    env.root_view_id = new_root_view_id;
}

// FIXME(ceg): core::open-new-buffer(path)
fn open_doc_controller_load_buffer(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) -> (usize, view::Id, bool) {
    // walk through buffer list/view
    // if ! already opened create new buffer + new view
    // show view

    // split code and reuse in main loader

    let controller_view = view.write();
    let text_view_view_id = controller_view.controlled_view.unwrap();
    let text_view = get_view_by_id(editor, text_view_view_id);
    let mut text_view = text_view.write();
    let odm = text_view.mode_ctx_mut::<OpenDocModeContext>("open-doc-mode");

    let path = if odm.completion_list.is_empty() {
        // create
        let s: String = odm.prompt.iter().collect();
        s
    } else {
        let mut path = odm.completion_list[odm.completion_index].clone();
        path.pop(); // remove ending \n
        path
    };

    dbg_println!("open-doc: try opening '{}'", path);

    let kind = path_to_buffer_kind(&path);

    let b = BufferBuilder::new(kind)
        .buffer_name(&path)
        .file_name(&path)
        .internal(false)
        .use_buffer_log(true)
        .finalize();

    // TODO: buffer id allocator fn
    let buffer_id = if let Some(b) = b {
        let buffer_id = b.read().id;
        editor.buffer_map.write().insert(buffer_id, b);
        buffer_id
    } else {
        return (env.root_view_index, env.root_view_id, false);
    };

    // configure buffer

    // TODO(ceg): move this to core:: as setup_buffer_modes(buffer)
    // per mode buffer metadata
    {
        let file_modes = editor.modes.clone();
        let dir_modes = editor.dir_modes.clone();

        let map = editor.buffer_map.clone();
        let mut map = map.write();

        for (_, buffer) in map.iter_mut() {
            let mut buffer = buffer.write();

            let modes = match buffer.kind {
                BufferKind::File => file_modes.borrow(),
                BufferKind::Directory => dir_modes.borrow(),
            };

            for (mode_name, mode) in modes.iter() {
                dbg_println!("setup mode[{}] buffer metadata", mode_name);
                let mut mode = mode.borrow_mut();
                mode.configure_buffer(editor, env, &mut buffer);
            }
        }
    }

    // pub fn create_view(editor: &mut Editor<'static>, env: &mut EditorEnv<'static>, buffer, modes) -> View<'static> {
    let buffer_map = editor.buffer_map.clone();
    let buffer_map = buffer_map.read();
    let buffer = buffer_map.get(&buffer_id).unwrap();
    let buffer = buffer.clone();

    // FIXME(ceg): there is a lot of copy paste from core::
    let json = parse_layout_str(DEFAULT_LAYOUT_JSON);
    if json.is_err() {
        dbg_print!("json parse error {:?}", json);
        return (0, view::Id(0), false);
    }
    let json = json.unwrap();

    let id = match kind {
        BufferKind::File => {
            build_view_layout_typed(&mut editor, &mut env, Some(buffer), &json, "file-view")
        }
        BufferKind::Directory => {
            build_view_layout_typed(&mut editor, &mut env, Some(buffer), &json, "dir-view")
        }
    };

    dbg_println!("open-doc : create view id {:?}", id);

    // a new top level view
    let idx = editor.root_views.len();
    let new_root_view_id = id.unwrap();
    editor.root_views.push(id.unwrap());

    let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

    // index buffers
    // TODO(ceg): send one event per doc
    if true {
        let msg = Message {
            seq: 0,
            input_ts: 0,
            ts,
            event: Event::IndexTask {
                buffer_map: Arc::clone(&editor.buffer_map),
            },
        };
        editor.indexer_tx.send(msg).unwrap_or(());
    }

    (idx, new_root_view_id, true)
}
