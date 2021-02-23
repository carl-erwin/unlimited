// Copyright (c) Carl-Erwin Griffith

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use crate::core::editor::ActionMap;
use crate::core::event::InputEvent;
use crate::core::event::InputEventMap;
use crate::core::event::InputEventRule;
use crate::core::view;

use crate::core::event::input_map::build_input_event_map;
use crate::core::event::input_map::DEFAULT_INPUT_MAP;

use crate::core::editor::build_core_action_map;

use std::marker::PhantomData;

// env.repeat_action_n , api to set repeat
// ctrl+:  -> minor mode to read repeat count
// esc -> reset repeat count
// kbr macro recording
pub struct EditorEnv<'a> {
    phantom: PhantomData<&'a u8>,
    pub graphic_display: bool,

    pub quit: bool,
    pub status: String, // TODO: move to test-mode

    pub action_map: ActionMap, // ref to current focused widget ?

    pub input_map: Rc<RefCell<InputEventMap>>,
    pub current_node: Option<Rc<InputEventRule>>,
    pub next_node: Option<Rc<InputEventRule>>,
    pub trigger: Vec<InputEvent>,

    pub pending_events: usize,

    //
    pub width: usize,
    pub height: usize,
    pub view_id: usize, // doc id in view

    // ADD view env ? TODO: refresh env after input_proessing

    //TODO: define workflow
    //  pre_input | input | post_input | pre_eval | eval | pos_eval |  pre_render | render | post_render
    //  each stage MUST have special signature
    // the modes register themselves at any stage
    // json ? to define pipeline ?
    //  mode dependencies ?
    //  xxx_mode: depends: "name", "name2", ...
    //  xxx_mode: optional_depends: "name", "name2", ...
    //
    // promote  FilterData like enum
    // promote  pub enum DataType {  FilterData }
    //
    // }
    //
    // define routes aka pipeline
    //
    // stage         pre_processing | processing | post processing
    //
    // pre_input    =
    // input        =
    // post_input   =
    // pre_eval     =
    // eval         =   TextMode::actions::*
    // pos_eval     =
    // pre_render   =   setup/reset ? here
    // render       =   TextMode::filters { raw | utf8 | highlight | tab | word-wrap | selection | marks | screen }
    // post_render  =   tooltips | syntax error | spellcheck
    // route

    // move ths to update_action
    // reset on each event handling
    pub view_pre_render: Vec<view::Action>,
    pub view_post_render: Vec<view::Action>,

    pub center_offset: Option<u64>,
    pub cur_mark_index: Option<usize>,
    pub max_offset: u64, // remove this, doc property

    pub draw_marks: bool,
}

impl<'a> EditorEnv<'a> {
    pub fn new() -> Self {
        let input_map = if let Ok(map) = build_input_event_map(DEFAULT_INPUT_MAP) {
            map
        } else {
            Rc::new(RefCell::new(HashMap::new()))
        };

        EditorEnv {
            phantom: PhantomData,
            graphic_display: false,
            quit: false,
            status: String::new(),
            action_map: build_core_action_map(),
            input_map,
            current_node: None,
            next_node: None,
            trigger: vec![],
            pending_events: 0,
            width: 0,
            height: 0,
            view_id: 0,
            view_pre_render: Vec::new(),
            view_post_render: Vec::new(),
            center_offset: None,
            cur_mark_index: None,
            max_offset: 0,
            draw_marks: true,
        }
    }
}
