use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use super::text_mode::TextModeContext;

use super::text_mode::PostInputAction;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;
use crate::core::modes::text_mode::mark::Mark;
use crate::core::view;
use crate::core::view::View;

use crate::core::view::ViewEvent;
use crate::core::view::ViewEventDestination;
use crate::core::view::ViewEventSource;

use crate::core::view::ControllerView;

static FIND_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+f" } ],    "action": "find:start" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+f" } ],    "action": "find:start-reverse" }
    ]
  }
]"#;

static FIND_INTERACTIVE_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "find:stop" },
     { "in": [{ "key": "BackSpace" } ], "action": "find:del-char" },
     { "in": [{ "key": "Delete" } ],    "action": "find:do-nothing" },
     { "in": [{ "key": "ctrl+f" } ],    "action": "find:next" },
     { "in": [{ "key": "ctrl+r" } ],    "action": "find:prev" },
     { "default": [],                   "action": "find:add-char" }
   ]
  }

]"#;

impl<'a> Mode for FindMode {
    fn name(&self) -> &'static str {
        &"find-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc find-mode ctx");
        let ctx = FindModeContext::new();
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
        let input_map = build_input_event_map(FIND_TRIGGER_MAP).unwrap();
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
            return;
        }

        let src_view_id = src_view.id;
        let svid = env.status_view_id.clone().unwrap();
        dbg_println!("find-mode svid = {:?}", svid);
        match event {
            &ViewEvent::ViewDeselected => {
                let mut fm = src_view.mode_ctx_mut::<FindModeContext>("find-mode");
                dbg_println!("find-mode fm.active = {}", fm.active);
                if fm.active {
                    let status_view = editor.view_map.get(&svid).unwrap();
                    let mut status_view = status_view.write();
                    match &mut status_view.controller {
                        Some(ControllerView { id, mode_name }) => {
                            if *id == src_view_id && *mode_name == "find-mode" {
                                clear_status_view(&mut status_view, &mut fm);
                            }
                        }

                        _ => {}
                    }
                    status_view.controller = None;
                }
            }

            &ViewEvent::ViewSelected => {
                let src_view_id = src_view.id;
                let mut fm = src_view.mode_ctx_mut::<FindModeContext>("find-mode");
                dbg_println!("find-mode fm.active = {}", fm.active);
                if fm.active {
                    let status_view = editor.view_map.get(&svid).unwrap();
                    let mut status_view = status_view.write();

                    status_view.controller = Some(view::ControllerView {
                        id: src_view_id,
                        mode_name: &"find-mode",
                    });
                    update_status_view(&mut status_view, &mut fm);
                }
            }

            _ => {}
        }
    }
}

pub struct FindModeContext {
    pub active: bool,
    pub reverse: bool,
    pub find_str: Vec<char>,
    pub match_start: Option<u64>,
    pub previous_encoded_str_len: usize,
}

impl FindModeContext {
    pub fn new() -> Self {
        dbg_println!("FindMode");
        FindModeContext {
            active: false,
            reverse: false,
            find_str: Vec::new(),
            match_start: None,
            previous_encoded_str_len: 0,
        }
    }
}
pub struct FindMode {
    // add common fields
}

impl FindMode {
    pub fn new() -> Self {
        dbg_println!("FindMode");
        FindMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "find:start", find_start);
        register_input_stage_action(&mut map, "find:start-reverse", find_start_reverse);
        register_input_stage_action(&mut map, "find:stop", find_stop);
        register_input_stage_action(&mut map, "find:add-char", find_add_char);
        register_input_stage_action(&mut map, "find:del-char", find_del_char);
        register_input_stage_action(&mut map, "find:next", find_next);
        register_input_stage_action(&mut map, "find:prev", find_prev);
    }
}

// cut and paste from display_find_string
fn update_status_view(status_view: &mut View, fm: &mut FindModeContext) {
    // clear status
    let doc = status_view.document().unwrap();
    let mut doc = doc.write();

    doc.delete_content(None);

    // set status text
    let text = "Find: ".as_bytes();
    doc.append(text);
    let w = status_view.width.saturating_sub(text.len() + 1);
    let s: String = fm.find_str.iter().collect();
    let d = &s.as_bytes()[s.len().saturating_sub(w)..];
    doc.append(d);
}

fn clear_status_view(status_view: &mut View, _fm: &mut FindModeContext) {
    // clear status
    let doc = status_view.document().unwrap();
    let mut doc = doc.write();
    // clear buffer. doc.erase_all();
    let sz = doc.size();
    doc.remove(0, sz, None);
}

// TODO(ceg): env.focus_stack.push(view.id)
// TODO(ceg): env.set_focus_to.push(status.id)
// Mode "find"
pub fn find_start(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
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
        let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
        let already_active = fm.active;
        fm.active = true;
        fm.reverse = false;

        status_view.write().controller = Some(view::ControllerView {
            id: v.id,
            mode_name: &"find-mode",
        });
        already_active
    };

    //
    let doc = status_view.read().document().unwrap();
    let mut doc = doc.write();

    // clear status view
    doc.delete_content(None);

    // set status text
    doc.append("Find: ".as_bytes());

    // push new input map for y/n
    let mut v = view.write();
    // lock focus on v
    // env.focus_locked_on = Some(v.id);

    // Do not input map push twice
    if !already_active {
        dbg_println!("configure find  {:?}", v.id);
        v.input_ctx.stack_pos = None;
        let input_map = build_input_event_map(FIND_INTERACTIVE_MAP).unwrap();
        let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push(("find-mode", input_map));
    }

    // TODO(ceg): add lock flag
    // to not exec lower input level
}

pub fn find_start_reverse(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let status_vid = view::get_status_view(&editor, &env, view);

    if status_vid.is_none() {
        // TODO(ceg): log missing status mode
        return;
    }

    let svid = status_vid.unwrap();

    let status_view = editor.view_map.get(&svid).unwrap();

    // start/resume ?
    {
        let vid = {
            let mut v = view.write();
            let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
            fm.active = true;
            fm.reverse = true;
            v.id
        };

        status_view.write().controller = Some(view::ControllerView {
            id: vid,
            mode_name: &"find-mode",
        });
    }

    //
    let doc = status_view.read().document().unwrap();
    let mut doc = doc.write();

    // clear status view
    doc.delete_content(None);

    // set status text
    doc.append("Find: ".as_bytes());

    // push new input map for y/n
    let mut v = view.write();
    // lock focus on v
    // env.focus_locked_on = Some(v.id);

    // TODO:
    dbg_println!("configure find  {:?}", v.id);
    v.input_ctx.stack_pos = None;
    let input_map = build_input_event_map(FIND_INTERACTIVE_MAP).unwrap();
    let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
    input_map_stack.push(("find-mode", input_map));
    // TODO(ceg): add lock flag
    // to not exec lower input level
}

pub fn find_stop(
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

        {
            let mut v = view.write();
            let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");

            // fm.reset();
            fm.find_str.clear();
            fm.match_start = None;
            fm.previous_encoded_str_len = 0;
            fm.active = false;
            //
            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.select_point.clear();
        }
    }
}

pub fn find_add_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut array = {
        let v = view.read();

        assert!(v.input_ctx.trigger.len() > 0);
        let idx = v.input_ctx.trigger.len() - 1;
        match &v.input_ctx.trigger[idx] {
            InputEvent::KeyPress {
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                key: Key::UnicodeArray(ref v),
            } => v.clone(), // should move Rc<> ?

            InputEvent::KeyPress {
                key: Key::Unicode(c),
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
            } => {
                vec![*c]
            }

            _ => {
                return;
            }
        }
    };

    let reverse = {
        let mut v = view.write();
        let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
        fm.find_str.append(&mut array);
        fm.reverse
    };

    display_find_string(&mut editor, &mut env, &view);

    if reverse {
        find_prev(&mut editor, &mut env, view);
    } else {
        find_next(&mut editor, &mut env, view);
    }
}

pub fn find_del_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let mut v = view.write();
        let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
        fm.find_str.pop();
        let offset = fm.match_start;
        fm.previous_encoded_str_len = 0;

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
        tm.select_point.clear();
        if let Some(offset) = offset {
            tm.marks[tm.mark_index].offset = offset;
        }
    }

    display_find_string(&mut editor, &mut env, &view);

    find_next(&mut editor, &mut env, view); // ?
}

pub fn find_next(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let mut v = view.write();
        let find_str = {
            let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
            fm.find_str.clone()
        };

        let mut encoded_str = vec![];

        {
            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            for c in find_str.iter() {
                let mut bin: [u8; 4] = [0; 4];
                let nr = codec.encode(*c as u32, &mut bin);
                for b in bin.iter().take(nr) {
                    encoded_str.push(*b);
                }
            }
        }

        dbg_println!("FIND encoded_str = {:?}", encoded_str);

        {
            let offset = {
                let mark_offset = {
                    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                    tm.marks[tm.mark_index].offset
                };

                let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
                if let Some(match_start) = fm.match_start {
                    let skip = if encoded_str.len() <= fm.previous_encoded_str_len {
                        1
                    } else {
                        0
                    };

                    match_start + skip
                } else {
                    mark_offset
                }
            };

            dbg_println!("FIND start @ offset = {:?}", offset);

            let doc = v.document().unwrap();
            let doc = doc.write();
            let offset = doc.find(&encoded_str, offset, None);
            dbg_println!("FIND offset = {:?}", offset);
            if let Some(offset) = offset {
                {
                    // save match start offset
                    let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
                    fm.match_start = Some(offset);
                    fm.previous_encoded_str_len = encoded_str.len();
                }

                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                tm.select_point.clear();
                tm.select_point.push(Mark { offset });
                tm.marks[tm.mark_index].offset = offset.saturating_add(encoded_str.len() as u64);
                tm.pre_compose_action
                    .push(PostInputAction::CenterAroundMainMarkIfOffScreen);
            }
        }
    }

    display_find_string(&mut editor, &mut env, &view);
}

pub fn find_prev(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let mut v = view.write();
        let find_str = {
            let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
            fm.find_str.clone()
        };

        let mut encoded_str = vec![];

        {
            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
            let codec = tm.text_codec.as_ref();

            for c in find_str.iter() {
                let mut bin: [u8; 4] = [0; 4];
                let nr = codec.encode(*c as u32, &mut bin);
                for b in bin.iter().take(nr) {
                    encoded_str.push(*b);
                }
            }
        }

        dbg_println!("FIND prev encoded_str = {:?}", encoded_str);

        {
            let offset = {
                let mark_offset = {
                    let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
                    tm.marks[tm.mark_index].offset
                };

                let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
                if let Some(match_start) = fm.match_start {
                    match_start
                } else {
                    mark_offset
                }
            };

            dbg_println!("FIND start @ offset = {:?}", offset);

            let doc = v.document().unwrap();
            let doc = doc.write();
            let offset = doc.find_reverse(&encoded_str, offset, None);
            dbg_println!("FIND offset = {:?}", offset);
            if let Some(offset) = offset {
                {
                    // save match start offset
                    let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
                    fm.match_start = Some(offset);
                    fm.previous_encoded_str_len = encoded_str.len();
                }

                let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

                tm.select_point.clear();
                tm.select_point.push(Mark { offset });
                tm.marks[tm.mark_index].offset = offset.saturating_add(encoded_str.len() as u64);
                tm.pre_compose_action
                    .push(PostInputAction::CenterAroundMainMarkIfOffScreen);
            }
        }
    }

    display_find_string(&mut editor, &mut env, &view);
}

pub fn display_find_string(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    //
    // reset status view : TODO(ceg): view::reset_status_view(&editor, view);
    let status_vid = view::get_status_view(&editor, &env, view);

    if let Some(status_vid) = status_vid {
        let mut v = view.write();
        let mut fm = v.mode_ctx_mut::<FindModeContext>("find-mode");

        let mut status_view = editor.view_map.get(&status_vid).unwrap().write();
        update_status_view(&mut status_view, &mut fm);
    }
}
