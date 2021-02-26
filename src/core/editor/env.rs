// Copyright (c) Carl-Erwin Griffith

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;
use std::time::Instant;

use crate::core::editor::ActionMap;
use crate::core::event::InputEvent;
use crate::core::event::InputEventMap;
use crate::core::event::InputEventRule;
use crate::core::view;

use crate::core::event::input_map::build_input_event_map;
use crate::core::event::input_map::DEFAULT_INPUT_MAP;

use crate::core::editor::build_core_action_map;

use std::marker::PhantomData;

// ADD view env ? TODO: refresh env after input_processing
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
//
// {READ}
// pre_input    =
// input        =
// post_input   =
//
// {EVAL}
// pre_eval     =
// eval         =   EVAL: TextMode::actions::*
// pos_eval     =
//
// {MODEL}
// pre_render   =   setup/reset ? here
// render       =   TextMode::filters { raw | utf8 | highlight | tab | word-wrap | selection | marks | screen }
// post_render  =   tooltips | syntax error | spellcheck
//
// pre_route_to_ui
// route_to_ui
// post_route_to_ui
// {PRINT}
//
// A mode is a collection functions+state registered
// at some stages
// Text indexers / Async ?
// connect the stage with tokio ?
// we have a model
//
// Processor : {PRE_PROCESS|PROCESS_|POST_PROCESS}
// input  core::DataType { core::events, mime_types }
// input  core::DataType
// each process receive an input with
// src/dest
// the destination can be diferrent from the input generator
// multiplex

// env.repeat_action_n , api to set repeat
// ctrl+:  -> minor mode to read repeat count
// esc -> reset repeat count
// kbr macro recording
pub struct EditorEnv<'a> {
    phantom: PhantomData<&'a u8>,
    pub graphic_display: bool,

    pub quit: bool,

    /// This flag is set when an input event as triggered a change
    /// and the ui must be refresh
    pub event_processed: bool,

    pub action_map: ActionMap, // ref to current focused widget ?

    pub input_map: Rc<RefCell<InputEventMap>>,
    pub current_node: Option<Rc<InputEventRule>>,
    pub next_node: Option<Rc<InputEventRule>>,
    pub trigger: Vec<InputEvent>,

    pub pending_events: usize,
    pub last_rdr_event: Instant,
    pub process_input_start: Instant,
    pub process_input_end: Instant,

    //
    pub width: usize,
    pub height: usize,
    pub view_id: usize, // doc id in view

    // move this to corresponding pre/pos stages
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

        // X11 session
        let graphic_display = match std::env::var("DISPLAY") {
            Ok(_) => true,
            Err(_) => false,
        };

        EditorEnv {
            phantom: PhantomData,
            graphic_display,
            quit: false,
            event_processed: false,

            action_map: build_core_action_map(),
            input_map,
            current_node: None,
            next_node: None,
            trigger: vec![],
            pending_events: 0,
            last_rdr_event: Instant::now(),
            process_input_start: Instant::now(),
            process_input_end: Instant::now(),
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
