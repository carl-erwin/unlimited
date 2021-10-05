use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use super::text_mode::TextModeContext;

use super::text_mode::Action;

use crate::core::editor::register_input_stage_action;
use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;
use crate::core::modes::text_mode::mark::Mark;
use crate::core::view;
use crate::core::view::View;

static FIND_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+f" } ],    "action": "find:start" }
    ]
  }
]"#;

static FIND_INTERACTIVE_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "find:stop" },
     { "in": [{ "key": "ctrl+g" } ],    "action": "find:stop" },
     { "in": [{ "key": "BackSpace" } ], "action": "find:del-char" },
     { "in": [{ "key": "Delete" } ],    "action": "find:do-nothing" },
     { "in": [{ "key": "ctrl+f" } ],    "action": "find:next" },
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
        input_map_stack.push(input_map);
    }
}

pub struct FindModeContext {
    // add common filed
    pub find_str: Vec<char>,
    pub match_start: Option<u64>,
    pub previous_encoded_str_len: usize,
}

impl FindModeContext {
    pub fn new() -> Self {
        dbg_println!("FindMode");
        FindModeContext {
            find_str: Vec::new(),
            match_start: None,
            previous_encoded_str_len: 0,
        }
    }
}
pub struct FindMode {
    // add common filed
}

impl FindMode {
    pub fn new() -> Self {
        dbg_println!("FindMode");
        FindMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "find:start", find_start);
        register_input_stage_action(&mut map, "find:stop", find_stop);
        register_input_stage_action(&mut map, "find:add-char", find_add_char);
        register_input_stage_action(&mut map, "find:del-char", find_del_char);
        register_input_stage_action(&mut map, "find:next", find_next);
    }
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

    if let Some(svid) = status_vid {
        let status_view = editor.view_map.get(&svid).unwrap();
        //
        let doc = status_view.read().document().unwrap();
        let mut doc = doc.write();
        // clear doc
        let sz = doc.size();
        doc.remove(0, sz, None);
        // set status text
        let text = "Find: ";
        let bytes = text.as_bytes();
        doc.insert(0, bytes.len(), &bytes);

        // push new input map for y/n
        {
            let mut v = view.write();
            // lock focus on v
            // env.focus_locked_on = Some(v.id);

            dbg_println!("configure find  {:?}", v.id);
            v.input_ctx.stack_pos = None;
            let input_map = build_input_event_map(FIND_INTERACTIVE_MAP).unwrap();
            let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(input_map);
            // TODO(ceg): add lock flag
            // to not exec lower input level
        }
    } else {
        // TODO(ceg): log missing status mode
    }
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

    {
        let mut v = view.write();
        let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
        fm.find_str.append(&mut array);
    }

    display_find_string(&mut editor, &mut env, &view);

    find_next(&mut editor, &mut env, view);
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
            let offset = doc.find(offset, &encoded_str);
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
                    .push(Action::CenterAroundMainMarkIfOffScreen);
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
        let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");

        let status_view = editor.view_map.get(&status_vid).unwrap();
        let doc = status_view.read().document().unwrap();
        let mut doc = doc.write();

        // clear buffer. doc.erase_all();
        let sz = doc.size();
        doc.remove(0, sz, None);

        // set status text
        let text = "Find: ";
        let bytes = text.as_bytes();
        doc.insert(0, bytes.len(), &bytes);

        let s: String = fm.find_str.iter().collect();
        // doc.append() ?
        let bytes = s.as_bytes();
        let sz = doc.size() as u64;
        doc.insert(sz, bytes.len(), &bytes);
    }
}
