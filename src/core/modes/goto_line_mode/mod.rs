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

use crate::core::document::find_nth_byte_offset;

static GOTO_LINE_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+g" } ],    "action": "goto-line:start" }
    ]
  }
]"#;

static GOTO_LINE_INTERACTIVE_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "goto-line:stop" },
     { "in": [{ "key": "\n" } ],        "action": "goto-line:stop" },
     { "in": [{ "key": "BackSpace" } ], "action": "goto-line:del-char" },
     { "in": [{ "key": "Delete" } ],    "action": "goto-line:do-nothing" },
     { "default": [],                   "action": "goto-line:add-char" }
   ]
  }

]"#;

impl<'a> Mode for GotoLineMode {
    fn name(&self) -> &'static str {
        &"goto-line-mode"
    }

    fn build_action_map(&self) -> InputStageActionMap<'static> {
        let mut map = InputStageActionMap::new();
        Self::register_input_stage_actions(&mut map);
        map
    }

    fn alloc_ctx(&self) -> Box<dyn Any> {
        dbg_println!("alloc goto-line-mode ctx");
        let ctx = GotoLineModeContext::new();
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
        let input_map = build_input_event_map(GOTO_LINE_TRIGGER_MAP).unwrap();
        let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
        input_map_stack.push(input_map);
    }
}

pub struct GotoLineModeContext {
    pub active: bool,
    pub goto_line_str: Vec<char>,
}

impl GotoLineModeContext {
    pub fn new() -> Self {
        dbg_println!("GotoLineMode");
        GotoLineModeContext {
            goto_line_str: Vec::new(),
        }
    }
}
pub struct GotoLineMode {
    // add common fields
}

impl GotoLineMode {
    pub fn new() -> Self {
        dbg_println!("GotoLineMode");
        GotoLineMode {}
    }

    pub fn register_input_stage_actions<'a>(mut map: &'a mut InputStageActionMap<'a>) {
        register_input_stage_action(&mut map, "goto-line:start", goto_line_start);
        register_input_stage_action(&mut map, "goto-line:stop", goto_line_stop);
        register_input_stage_action(&mut map, "goto-line:add-char", goto_line_add_char);
        register_input_stage_action(&mut map, "goto-line:del-char", goto_line_del_char);
    }
}

pub fn goto_line_start(
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
    //
    let doc = status_view.read().document().unwrap();
    let mut doc = doc.write();

    // clear status view
    doc.delete_content(None);

    // set status text
    let text = "Goto: ";
    let bytes = text.as_bytes();
    doc.insert(0, bytes.len(), &bytes);

    // setup new input map
    let mut v = view.write();
    v.input_ctx.stack_pos = None;
    let input_map = build_input_event_map(GOTO_LINE_INTERACTIVE_MAP).unwrap();
    let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
    input_map_stack.push(input_map);
}

pub fn goto_line_stop(
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
            let gtm = v.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");

            // gtm.reset();
            gtm.goto_line_str.clear();
        }
    }
}

pub fn goto_line_set_target_line(view: &Rc<RwLock<View<'static>>>, target_line: u64) {
    let doc = view.read().document().unwrap();
    let doc = doc.read();

    let max_offset = doc.size();

    let mut v = view.write();

    let offset = if target_line <= 1 {
        0
    } else {
        let line_number = target_line.saturating_sub(1);
        if let Some(offset) = find_nth_byte_offset(&doc, '\n' as u8, line_number) {
            offset + 1
        } else {
            max_offset as u64
        }
    };

    {
        v.start_offset = offset;

        let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");

        tm.marks.clear();
        tm.marks.push(Mark { offset });
        tm.mark_index = 0;

        tm.pre_compose_action.push(Action::CenterAroundMainMark);
    }
}

pub fn goto_line_add_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut array = vec![];

    let goto_line_str_size = {
        let v = view.read();
        let gtm = v.mode_ctx::<GotoLineModeContext>("goto-line-mode");
        gtm.goto_line_str.len()
    };

    // filter input event
    {
        let v = view.read();

        assert!(v.input_ctx.trigger.len() > 0);
        let idx = v.input_ctx.trigger.len() - 1;
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
                if *c >= '0' && *c <= '9' {
                    if (goto_line_str_size == 0 && *c != '0') || goto_line_str_size > 0 {
                        array.push(*c);
                    }
                }
            }

            _ => {
                return;
            }
        }

        if array.is_empty() {
            return;
        }
    }

    // compute target line number
    let target_line = {
        let mut v = view.write();

        let size = {
            let doc = v.document().unwrap();
            let doc = doc.read();
            doc.size() as u64
        };

        let gtm = v.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");

        gtm.goto_line_str.append(&mut array);

        let line_str: String = gtm.goto_line_str.iter().collect();

        let n = line_str.parse::<u64>().unwrap_or(0);

        if n > size {
            gtm.goto_line_str.pop();
            return;
        }

        n
    };

    display_goto_line_string(&mut editor, &mut env, &view);

    goto_line_set_target_line(&view, target_line);
}

pub fn goto_line_del_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let target_line = {
        let mut v = view.write();
        let gtm = v.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");
        if gtm.goto_line_str.is_empty() {
            return;
        }
        gtm.goto_line_str.pop();
        let line_str: String = gtm.goto_line_str.iter().collect();
        line_str.parse::<u64>().unwrap_or(0)
    };

    display_goto_line_string(&mut editor, &mut env, &view);

    goto_line_set_target_line(&view, target_line);
}

pub fn display_goto_line_string(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    //
    // reset status view : TODO(ceg): view::reset_status_view(&editor, view);
    let status_vid = view::get_status_view(&editor, &env, view);

    if let Some(status_vid) = status_vid {
        let mut v = view.write();
        let gtm = v.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");

        let status_view = editor.view_map.get(&status_vid).unwrap();
        let doc = status_view.read().document().unwrap();
        let mut doc = doc.write();

        // clear buffer. doc.erase_all();
        let sz = doc.size();
        doc.remove(0, sz, None);

        // set status text
        let text = "Goto: ";
        let bytes = text.as_bytes();
        doc.insert(0, bytes.len(), &bytes);

        let s: String = gtm.goto_line_str.iter().collect();
        // doc.append() ?
        let bytes = s.as_bytes();
        let sz = doc.size() as u64;
        doc.insert(sz, bytes.len(), &bytes);
    }
}
