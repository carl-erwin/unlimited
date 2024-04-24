use std::time::Instant;

use std::marker::PhantomData;

use crate::core::view;

// ADD view env ? TODO(ceg): refresh env after input_processing
//TODO(ceg): define workflow
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
// the destination can be different from the input generator
// multiplex

// env.repeat_action_n , api to set repeat
// ctrl+:  -> minor mode to read repeat count
// esc -> reset repeat count
// kbr macro recording
#[derive(Debug)]
pub struct EditorEnv<'a> {
    phantom: PhantomData<&'a u8>,

    /// set this if the editor runs in a graphical terminal emulator
    pub graphic_display: bool,

    /// set this to qui th emulator (no checks)<br/>
    pub quit: bool,

    /// the last received input event
    pub current_input_event: crate::core::event::InputEvent,

    /// This flag is set when an input event as triggered a change
    /// and the ui must be refreshed
    pub refresh_ui: bool,

    pub pending_events: usize,
    pub last_rdr_event: Instant,
    pub current_time: Instant,
    pub process_input_start: Instant,
    pub process_input_end: Instant,

    // the root view width
    pub width: usize,
    // the root view height
    pub height: usize,
    // TODO(ceg): maintain per view
    pub global_x: Option<i32>,
    pub global_y: Option<i32>,
    pub local_x: Option<i32>,
    pub local_y: Option<i32>,

    pub diff_x: i32,
    pub diff_y: i32,

    //
    pub root_view_index: usize,

    //
    pub prev_view_id: view::Id,
    pub root_view_id: view::Id,
    /// the view receiving the keyboard inputs
    pub active_view: Option<view::Id>,

    /// the view the pointer is on
    pub pointer_over_view_id: view::Id,

    pub last_selected_view_id: view::Id,

    /// the view the pointer is on
    pub focus_locked_on_view_id: Option<view::Id>,

    /// this view takes all input events (pointer/key/button press/etc..)
    pub grab_view: Option<view::Id>,

    //
    pub status_view_id: Option<view::Id>,

    pub center_offset: Option<u64>,

    // stages stats
    pub time_spent: [[u128; 3]; 3],

    pub input_ts: u128,
}

impl<'a> EditorEnv<'a> {
    pub fn new() -> Self {
        // FIXME:
        let graphic_display = match std::env::var("TERM") {
            Ok(name) => match name.as_ref() {
                "linux" => false,
                "vt100" | "xterm-256color" => true,
                _ => false,
            },
            Err(_) => false,
        };

        EditorEnv {
            phantom: PhantomData,
            graphic_display,
            quit: false,
            current_input_event: crate::core::event::InputEvent::NoInputEvent,
            refresh_ui: false,
            pending_events: 0,
            last_rdr_event: Instant::now(),
            current_time: Instant::now(),
            process_input_start: Instant::now(),
            process_input_end: Instant::now(),
            width: 0,
            height: 0,
            // event coordinates
            global_x: None,
            global_y: None,
            local_x: None,
            local_y: None,
            diff_x: 0,
            diff_y: 0,
            //max
            root_view_index: 0,
            prev_view_id: view::Id(1), // NB
            root_view_id: view::Id(1), // NB
            center_offset: None,
            active_view: None,
            grab_view: None,
            pointer_over_view_id: view::Id(0),
            last_selected_view_id: view::Id(0),
            focus_locked_on_view_id: None,
            status_view_id: None,
            time_spent: [[0, 0, 0], [0, 0, 0], [0, 0, 0]],
            input_ts: 0,
        }
    }
}
