use std::any::Any;

use parking_lot::RwLock;

use std::rc::Rc;

use super::Mode;

use super::text_mode::TextModeContext;

use super::text_mode::PostInputAction;

use crate::core::document::DocumentBuilder;
use crate::core::editor::register_input_stage_action;
use crate::core::editor::set_focus_on_vid;
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

use crate::core::modes::text_mode::center_view_around_offset;

static FIND_TRIGGER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "ctrl+f" } ],    "action": "find:start" },
     { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+f" } ],    "action": "find:start-reverse" }
    ]
  }
]"#;

static FIND_CONTROLLER_MAP: &str = r#"
[
  {
    "events": [
     { "in": [{ "key": "Escape" } ],    "action": "find:stop" },
     { "in": [{ "key": "\n" } ],        "action": "find:stop" },
     { "in": [{ "key": "ctrl+g" } ],    "action": "find:stop" },
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
        editor: &mut Editor<'static>,
        env: &mut EditorEnv<'static>,
        view: &mut View<'static>,
    ) {
        dbg_println!("configure find  {:?}", view.id);

        // setup input map for core actions
        {
            let input_map = build_input_event_map(FIND_TRIGGER_MAP).unwrap();
            let mut input_map_stack = view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push((self.name(), input_map));
        }

        // add controller
        create_find_controller_view(editor, env, view);
    }
}

pub struct FindModeContext {
    pub active: bool,
    pub reverse: bool,
    pub find_str: Vec<char>,
    pub match_start: Option<u64>,
    pub match_end: Option<u64>,
    pub previous_encoded_str_len: usize,
    pub controller_view_id: view::Id,
}

impl FindModeContext {
    pub fn new() -> Self {
        dbg_println!("FindMode");
        FindModeContext {
            active: false,
            reverse: false,
            find_str: Vec::new(),
            match_start: None,
            match_end: None,
            previous_encoded_str_len: 0,
            controller_view_id: view::Id(0),
        }
    }
    pub fn reset(&mut self) -> &mut Self {
        self.find_str.clear();
        self.match_start = None;
        self.match_end = None;
        self.previous_encoded_str_len = 0;
        self.active = false;
        self
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
    }
}

pub fn find_start(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let status_vid = view::get_status_view(&editor, &env, view);
    if status_vid.is_none() {
        // TODO(ceg): log missing status mode / panic!("")
        return;
    }

    // start/resume ?
    let controller_id = {
        let mut v = view.write();
        let fm = v.mode_ctx_mut::<FindModeContext>("find-mode");
        fm.active = true;

        let id = fm.controller_view_id;

        // attach to status view
        let controller = editor.view_map.get(&id).unwrap();
        controller.write().parent_id = Some(status_vid.unwrap());
        v.controller = Some(ControllerView {
            id,
            mode_name: &"find-mode",
        });

        id
    };

    find_show_controller_view(editor, env, view);
    set_focus_on_vid(editor, env, controller_id);
}

pub fn find_controller_stop(
    editor: &mut Editor<'static>,
    env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let status_vid = env.status_view_id.unwrap();
        let mut status_view = editor.view_map.get(&status_vid).unwrap().write();
        status_view.layout_direction = LayoutDirection::Horizontal;
        // if last == expected id
        status_view.children.pop(); // replace previous Child
    }

    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        {
            let mut text_view = editor.view_map.get(&text_view_id).unwrap().write();
            text_view.controller = None;

            let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");
            fm.reset();

            //
            let doc = v.document().unwrap();
            let mut doc = doc.write();
            doc.delete_content(None);
            doc.append("Find: ".as_bytes());
        }

        // set input focus to
        set_focus_on_vid(editor, env, text_view_id);
    }
}

fn create_find_controller_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &mut View,
) {
    // get status vid -> status_vid

    // (w,h) = status_vid.dimension()
    let (x, y) = (0, 0);
    let (w, h) = (1, 1);

    let doc = DocumentBuilder::new()
        .document_name("find-controller")
        .internal(true)
        .use_buffer_log(false)
        .finalize();

    {
        doc.as_ref().unwrap().write().append("Find: ".as_bytes());
    }

    // create view at mode creation
    let mut controller_view = View::new(
        &mut editor,
        &mut env,
        None,
        (x, y),
        (w, h),
        doc,
        &vec!["status-mode".to_owned()], // TODO(ceg): find-controller
        0,
    );

    controller_view.ignore_focus = false;

    controller_view.controlled_view = Some(view.id);

    // set controller target as view.id
    let mut fm = view.mode_ctx_mut::<FindModeContext>("find-mode");

    fm.controller_view_id = controller_view.id;

    dbg_println!("fm.controller_view_id = {:?}", fm.controller_view_id);

    // setup new input map
    {
        controller_view.input_ctx.stack_pos = None;

        {
            let event_map = build_input_event_map(FIND_CONTROLLER_MAP).unwrap();
            let mut input_map_stack = controller_view.input_ctx.input_map.as_ref().borrow_mut();
            input_map_stack.push(("find-controller", event_map));
        }

        let mut action_map = InputStageActionMap::new();

        register_input_stage_action(&mut action_map, "find:stop", find_controller_stop);
        register_input_stage_action(&mut action_map, "find:add-char", find_controller_add_char);
        register_input_stage_action(&mut action_map, "find:del-char", find_controller_del_char);
        register_input_stage_action(&mut action_map, "find:next", find_controller_next);
        register_input_stage_action(&mut action_map, "find:prev", find_controller_prev);

        controller_view.register_action_map(action_map);
    }

    editor.add_view(controller_view.id, Rc::new(RwLock::new(controller_view)));
}

fn find_show_controller_view(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    text_view: &Rc<RwLock<View<'static>>>,
) {
    let status_vid = env.status_view_id.unwrap();

    let mut status_view = editor.view_map.get(&status_vid).unwrap().write();

    status_view.layout_direction = LayoutDirection::Horizontal;

    let text_view = text_view.read();
    let fm = text_view.mode_ctx::<FindModeContext>("find-mode");

    status_view.children.pop(); // replace previous child
    status_view.children.push(ChildView {
        id: fm.controller_view_id,
        layout_op: LayoutOperation::Percent { p: 100.0 },
    });
}

pub fn find_controller_add_char(
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
        let v = view.read();

        if let Some(text_view_id) = v.controlled_view {
            let mut text_view = editor.view_map.get(&text_view_id).unwrap().write();
            let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");
            fm.find_str.append(&mut array);
            fm.reverse
        } else {
            return;
        }
    };

    display_find_string(&mut editor, &mut env, &view);

    if reverse {
        find_controller_prev(&mut editor, &mut env, view);
    } else {
        find_controller_next(&mut editor, &mut env, view);
    }
}

pub fn find_controller_del_char(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    {
        let v = view.read();
        if let Some(text_view_id) = v.controlled_view {
            let mut text_view = editor.view_map.get(&text_view_id).unwrap().write();
            let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");
            fm.find_str.pop();
            let offset = fm.match_start;
            fm.previous_encoded_str_len = 0;

            let tm = text_view.mode_ctx_mut::<TextModeContext>("text-mode");
            tm.select_point.clear();
            if let Some(offset) = offset {
                tm.marks[tm.mark_index].offset = offset;
            }
        } else {
            // panic!
            return;
        }
    }

    display_find_string(&mut editor, &mut env, &view);

    find_controller_next(&mut editor, &mut env, view); // ?
}

pub fn find_controller_next(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut center_around_offset = None;

    {
        let v = view.read();
        let find_str = {
            if let Some(text_view_id) = v.controlled_view {
                let text_view = editor.view_map.get(&text_view_id).unwrap().read();
                let fm = text_view.mode_ctx::<FindModeContext>("find-mode");
                fm.find_str.clone()
            } else {
                return;
            }
        };

        let mut encoded_str = vec![];

        {
            if let Some(text_view_id) = v.controlled_view {
                let text_view = editor.view_map.get(&text_view_id).unwrap().read();
                let tm = text_view.mode_ctx::<TextModeContext>("text-mode");
                let codec = tm.text_codec.as_ref();

                for c in find_str.iter() {
                    let mut bin: [u8; 4] = [0; 4];
                    let nr = codec.encode(*c as u32, &mut bin);
                    for b in bin.iter().take(nr) {
                        encoded_str.push(*b);
                    }
                }
            } else {
                return;
            }
        }

        dbg_println!("FIND encoded_str = {:?}", encoded_str);

        {
            let offset = {
                let mark_offset = {
                    let text_view_id = v.controlled_view.as_ref().unwrap();
                    let text_view = editor.view_map.get(text_view_id).unwrap().read();
                    let tm = text_view.mode_ctx::<TextModeContext>("text-mode");
                    tm.marks[tm.mark_index].offset
                };

                let text_view_id = v.controlled_view.as_ref().unwrap();
                let text_view = editor.view_map.get(text_view_id).unwrap().read();
                let fm = text_view.mode_ctx::<FindModeContext>("find-mode");
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

            let text_view_id = v.controlled_view.as_ref().unwrap();
            let mut text_view = editor.view_map.get(text_view_id).unwrap().write();

            let doc = text_view.document().unwrap();
            let doc = doc.write();
            let offset = doc.find(&encoded_str, offset, None);
            dbg_println!("FIND offset = {:?}", offset);
            if let Some(offset) = offset {
                {
                    // save match start offset
                    let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");
                    fm.previous_encoded_str_len = encoded_str.len();

                    fm.match_start = Some(offset);
                    fm.match_end = Some(offset.saturating_add(encoded_str.len() as u64));

                    dbg_println!("FIND next match_start = {:?}", fm.match_start);
                    dbg_println!("FIND next match_end = {:?}", fm.match_end);

                    // TODO(ceg): controller -> text view -> center around mark
                    // push to editor special PostInputAction que
                    // editor.pre_compose_action.push(PostInputAction::CenterViewAroundMainMarkIfOffScreen { view_id, offset} );
                    if !text_view.screen.read().contains_offset(offset) {
                        center_around_offset = Some(offset);
                    }
                }

                let tm = text_view.mode_ctx_mut::<TextModeContext>("text-mode");

                tm.select_point.clear();
                tm.select_point.push(Mark { offset });
                tm.marks[tm.mark_index].offset = offset.saturating_add(encoded_str.len() as u64);
            }
        }
    }

    if let Some(offset) = center_around_offset {
        let v = view.read();
        let text_view_id = v.controlled_view.as_ref().unwrap();
        let text_view = editor.view_map.get(text_view_id).unwrap().clone();
        center_view_around_offset(&text_view, editor, env, offset);
    }

    display_find_string(&mut editor, &mut env, &view);
}

pub fn find_controller_prev(
    mut editor: &mut Editor<'static>,
    mut env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let mut center_around_offset = None;

    {
        let v = view.read();
        let find_str = {
            let text_view_id = v.controlled_view.as_ref().unwrap();
            let text_view = editor.view_map.get(text_view_id).unwrap().read();
            let fm = text_view.mode_ctx::<FindModeContext>("find-mode");
            fm.find_str.clone()
        };

        let mut encoded_str = vec![];

        {
            let text_view_id = v.controlled_view.as_ref().unwrap();
            let text_view = editor.view_map.get(text_view_id).unwrap().read();
            let tm = text_view.mode_ctx::<TextModeContext>("text-mode");
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
                let text_view_id = v.controlled_view.as_ref().unwrap();
                let mut text_view = editor.view_map.get(text_view_id).unwrap().write();

                let mark_offset = {
                    let tm = text_view.mode_ctx_mut::<TextModeContext>("text-mode");
                    tm.marks[tm.mark_index].offset
                };

                let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");

                dbg_println!("FIND prev match_start = {:?}", fm.match_start);
                dbg_println!("FIND prev match_end = {:?}", fm.match_end);

                if let Some(match_start) = fm.match_start {
                    match_start.saturating_sub(1)
                } else {
                    mark_offset
                }
            };

            dbg_println!("FIND PREV start @ offset = {:?}", offset);

            let text_view_id = v.controlled_view.as_ref().unwrap();
            let mut text_view = editor.view_map.get(text_view_id).unwrap().write();
            let doc = text_view.document().unwrap();
            let doc = doc.write();

            let offset = doc.find_reverse(&encoded_str, offset, None);
            dbg_println!("FIND PREV offset = {:?}", offset);
            if let Some(offset) = offset {
                {
                    // save match start offset
                    let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");
                    fm.match_start = Some(offset);
                    fm.previous_encoded_str_len = encoded_str.len();
                }

                let tm = text_view.mode_ctx_mut::<TextModeContext>("text-mode");

                tm.select_point.clear();
                tm.select_point.push(Mark { offset });
                tm.marks[tm.mark_index].offset = offset.saturating_add(encoded_str.len() as u64);

                // TODO(ceg): controller -> text view -> center around mark
                // push to editor special PostInputAction que
                // editor.pre_compose_action.push(PostInputAction::CenterViewAroundMainMarkIfOffScreen { view_id, offset} );
                if !text_view.screen.read().contains_offset(offset) {
                    center_around_offset = Some(offset);
                }
            }
        }
    }

    if let Some(offset) = center_around_offset {
        let v = view.read();
        let text_view_id = v.controlled_view.as_ref().unwrap();
        let text_view = editor.view_map.get(text_view_id).unwrap().clone();
        center_view_around_offset(&text_view, editor, env, offset);
    }

    display_find_string(&mut editor, &mut env, &view);
}

pub fn display_find_string(
    editor: &mut Editor<'static>,
    _env: &mut EditorEnv<'static>,
    view: &Rc<RwLock<View<'static>>>,
) {
    let v = view.read();
    if let Some(text_view_id) = v.controlled_view {
        let mut text_view = editor.view_map.get(&text_view_id).unwrap().write();
        let fm = text_view.mode_ctx_mut::<FindModeContext>("find-mode");

        // build find string
        let doc = v.document().unwrap();
        let mut doc = doc.write();
        doc.delete_content(None);
        doc.append("Find: ".as_bytes());

        let s: String = fm.find_str.iter().collect();
        doc.append(s.as_bytes());
    } else {
        // panic! ?
        return;
    }
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
    let input_map = build_input_event_map(FIND_CONTROLLER_MAP).unwrap();
    let mut input_map_stack = v.input_ctx.input_map.as_ref().borrow_mut();
    input_map_stack.push(("find-mode", input_map));
    // TODO(ceg): add lock flag
    // to not exec lower input level
}
