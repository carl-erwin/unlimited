use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use super::text_mode::TextModeContext;

use super::text_mode::PostInputAction;

use crate::core::buffer::get_byte_count;

use crate::core::buffer::BufferBuilder;
use crate::core::buffer::BufferKind;

use crate::core::editor::get_view_by_id;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_view_id;

use crate::core::editor::InputStageActionMap;
use crate::core::Editor;
use crate::core::EditorEnv;

use crate::core::event::*;

use crate::core::event::input_map::build_input_event_map;
use crate::core::modes::text_mode::mark::Mark;
use crate::core::view;
use crate::core::view::ChildView;
use crate::core::view::View;

use crate::core::view::ControllerView;
use crate::core::view::LayoutDirection;
use crate::core::view::LayoutOperation;

use crate::core::buffer::find_nth_byte_offset;

use super::text_mode::run_text_mode_actions_vec;

static GOTO_LINE_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+g" } ],    "action": "goto-line:start" }
    ]
  }
]"#;

static GOTO_LINE_CONTROLLER_INTERACTIVE_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "goto-line-controller:stop" },
     { "in": [{ "key": "\n" } ],        "action": "goto-line-controller:stop" },
     { "in": [{ "key": "ctrl+g" } ],    "action": "goto-line-controller:stop" },
     { "in": [{ "key": "ctrl+q" } ],    "action": "goto-line-controller:stop" },
     { "in": [{ "key": "BackSpace" } ], "action": "goto-line-controller:del-char" },
     { "default": [],                   "action": "goto-line-controller:add-char" }
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
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        dbg_println!("configure find  {:?}", view.id);

        // setup input map for core actions
        {
            let input_map = build_input_event_map(GOTO_LINE_TRIGGER_MAP).unwrap();
            let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push((self.name(), input_map));
        }

        // add controller
        create_goto_line_controller_view(editor, env, view);
    }
}

pub struct GotoLineModeContext {
    pub active: bool,
    pub eof: bool,
    pub goto_line_str: Vec<char>,
    pub controller_view_id: view::Id,
}

impl GotoLineModeContext {
    pub fn new() -> Self {
        dbg_println!("GotoLineMode");
        GotoLineModeContext {
            active: false,
            eof: false,
            goto_line_str: Vec::new(),
            controller_view_id: view::Id(0),
        }
    }

    pub fn reset(&mut self) -> &mut Self {
        self.goto_line_str.clear();
        self.eof = false;
        self.active = false;
        self
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
    }
}

pub fn goto_line_start(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let status_view_id = view::get_status_view(editor, env, view);
    if status_view_id.is_none() {
        // TODO(ceg): log missing status mode / panic!("")
        return;
    }

    // start/resume ?
    let controller_id = {
        let mut v = view.write();
        let gtm = v.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");
        gtm.active = true;

        let id = gtm.controller_view_id;

        // attach to status view
        let controller = get_view_by_id(editor, id);
        controller.write().parent_id = Some(status_view_id.unwrap());

        v.controller = Some(ControllerView {
            id: gtm.controller_view_id,
            mode_name: &"goto-line-mode",
        });

        id
    };

    goto_line_show_controller_view(editor, env, view);
    set_focus_on_view_id(&mut editor, &mut env, controller_id);
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
    let status_view_id = view::get_status_view(editor, env, view);
    if let Some(status_view_id) = status_view_id {
        let status_view = get_view_by_id(editor, status_view_id);
        let buffer = status_view.read().buffer().unwrap();
        let mut buffer = buffer.write();
        // clear buffer
        let sz = buffer.size();
        buffer.remove(0, sz, None);

        {
            let mut v = view.write();
            let gtm = v.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");

            gtm.reset();
        }
    }
}

fn create_goto_line_controller_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &mut View,
) {
    // get status vid -> status_view_id

    // (w,h) = status_view_id.dimension()
    let (x, y) = (0, 0);
    let (w, h) = (1, 1);

    let buffer = BufferBuilder::new(BufferKind::File)
        .buffer_name("goto-controller")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    {
        buffer.as_ref().unwrap().write().append("Goto: ".as_bytes());
    }

    // create view at mode creation
    let mut controller_view = View::new(
        &mut editor,
        &mut env,
        None,
        (x, y),
        (w, h),
        buffer,
        &vec!["status-mode".to_owned()], // TODO(ceg): goto-line-controller
        0,
    );

    controller_view.ignore_focus = false;

    controller_view.controlled_view = Some(view.id);

    // set controller target as view.id
    let gtm = view.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");

    gtm.controller_view_id = controller_view.id;

    dbg_println!("gtm.controller_view_id = {:?}", gtm.controller_view_id);

    // setup new input map
    {
        controller_view.input_ctx.stack_pos = None;

        {
            let event_map = build_input_event_map(GOTO_LINE_CONTROLLER_INTERACTIVE_MAP).unwrap();
            let mut input_map_stack = controller_view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(("goto-line-controller", event_map));
        }

        let mut action_map = InputStageActionMap::new();

        register_input_stage_action(
            &mut action_map,
            "goto-line-controller:stop",
            goto_line_controller_stop,
        );
        register_input_stage_action(
            &mut action_map,
            "goto-line-controller:add-char",
            goto_line_controller_add_char,
        );
        register_input_stage_action(
            &mut action_map,
            "goto-line-controller:del-char",
            goto_line_controller_del_char,
        );

        controller_view.register_action_map(action_map);
    }

    editor.add_view(controller_view.id, controller_view);
}

fn goto_line_show_controller_view(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    text_view: &Rc<RwLock<View<'static>>>,
) {
    let status_view_id = env.status_view_id.unwrap();

    let status_view = get_view_by_id(editor, status_view_id);
    let mut status_view = status_view.write();

    status_view.layout_direction = LayoutDirection::Horizontal;

    let text_view = text_view.read();
    let gtm = text_view.mode_ctx::<GotoLineModeContext>("goto-line-mode");

    status_view.children.pop(); // replace previous child
    status_view.children.push(ChildView {
        id: gtm.controller_view_id,
        layout_op: LayoutOperation::Percent { p: 100.0 },
    });
}

pub fn goto_line_controller_add_char(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut array = vec![];

    let goto_line_str_size = {
        let v = view.read();

        if let Some(text_view_id) = v.controlled_view {
            let text_view = get_view_by_id(editor, text_view_id);
            let text_view = text_view.read();

            let gtm = text_view.mode_ctx::<GotoLineModeContext>("goto-line-mode");
            gtm.goto_line_str.len()
        } else {
            return;
        }
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
                if (*c >= '0' && *c <= '9')
                    && ((goto_line_str_size == 0 && *c != '0') || goto_line_str_size > 0)
                {
                    array.push(*c);
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
        let v = view.read();
        if let Some(text_view_id) = v.controlled_view {
            let text_view = get_view_by_id(editor, text_view_id);
            let mut text_view = text_view.write();

            let buffer = text_view.buffer().unwrap();

            // do this at start and store in gtm
            let nb_lines = get_byte_count(&buffer.read(), '\n' as usize).unwrap_or(0);

            let gtm = text_view.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");
            if gtm.eof {
                return;
            }

            gtm.goto_line_str.append(&mut array);

            let line_str: String = gtm.goto_line_str.iter().collect();

            dbg_println!("nb_lines = {:?}", nb_lines);

            let n = line_str.parse::<u64>().unwrap_or(0);

            dbg_println!("goto line target {}", n);

            // render line number
            let buffer = v.buffer().unwrap();
            let mut buffer = buffer.write();
            buffer.delete_content(None);
            buffer.append("Goto: ".as_bytes());

            let s: String = gtm.goto_line_str.iter().collect();
            buffer.append(s.as_bytes());
            gtm.eof = n > nb_lines;
            std::cmp::min(n, nb_lines + 1)
        } else {
            return;
        }
    };

    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        let text_view = get_view_by_id(editor, text_view_id);
        goto_line_set_target_line(editor, env, &text_view, target_line);
    }
}

// refactor add+del (Some(array))
pub fn goto_line_controller_del_char(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    // compute target line number
    let target_line = {
        let v = view.read();
        if let Some(text_view_id) = v.controlled_view {
            let text_view = get_view_by_id(editor, text_view_id);
            let mut text_view = text_view.write();

            let buffer = text_view.buffer().unwrap();

            // do this at start and store in gtm
            let nb_lines = get_byte_count(&buffer.read(), '\n' as usize).unwrap_or(0);

            let gtm = text_view.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");

            gtm.goto_line_str.pop();

            let line_str: String = gtm.goto_line_str.iter().collect();

            dbg_println!("nb_lines = {:?}", nb_lines);

            let n = line_str.parse::<u64>().unwrap_or(0);

            gtm.eof = n > nb_lines;

            dbg_println!("goto line target {}", n);

            // render line number
            let buffer = v.buffer().unwrap();
            let mut buffer = buffer.write();
            buffer.delete_content(None);
            buffer.append("Goto: ".as_bytes());

            let s: String = gtm.goto_line_str.iter().collect();
            buffer.append(s.as_bytes());
            if n > nb_lines {
                return;
            }
            n
        } else {
            return;
        }
    };

    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        let text_view = get_view_by_id(editor, text_view_id);
        goto_line_set_target_line(editor, env, &text_view, target_line);
    }
}

pub fn goto_line_controller_stop(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let status_view_id = env.status_view_id.unwrap();
        let status_view = get_view_by_id(editor, status_view_id);
        let mut status_view = status_view.write();

        status_view.layout_direction = LayoutDirection::Horizontal;
        // if last == expected id
        status_view.children.pop(); // replace previous Child
    }

    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        {
            let text_view = get_view_by_id(editor, text_view_id);
            let mut text_view = text_view.write();

            text_view.controller = None;

            let gtm = text_view.mode_ctx_mut::<GotoLineModeContext>("goto-line-mode");
            gtm.reset();

            //
            let buffer = v.buffer().unwrap();
            let mut buffer = buffer.write();
            buffer.delete_content(None);
            buffer.append("Goto: ".as_bytes());
        }

        // set input focus to
        set_focus_on_view_id(editor, env, text_view_id);
    }
}

pub fn goto_line_set_target_line(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
    target_line: u64,
) {
    {
        let buffer = view.read().buffer().unwrap();
        let buffer = buffer.read();
        let max_offset = buffer.size();
        let mut v = view.write();
        let offset = if target_line <= 1 {
            0
        } else {
            let line_number = target_line.saturating_sub(1);
            if let Some(offset) = find_nth_byte_offset(&buffer, '\n' as u8, line_number) {
                offset + 1
            } else {
                max_offset as u64
            }
        };

        {
            // if offscreen // user option goto-line-mode:always-center-around-line = true ?
            if !v.screen.read().contains_offset(offset) {
                v.start_offset = offset;
            }

            let tm = v.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.marks.clear();
            tm.marks.push(Mark { offset });
            tm.mark_index = 0;
        }
    }

    {
        run_text_mode_actions_vec(
            &mut editor,
            &mut env,
            &view,
            &vec![PostInputAction::CenterAroundMainMark],
        );
    }
}
