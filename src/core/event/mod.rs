// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

pub mod input_map;

use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

use crate::core::document;
use crate::core::screen::Screen;
use crate::core::view;

use std::sync::atomic::{AtomicUsize, Ordering};

////////////////
// TODO: implement functions ti update the counters
// on send (++) / receive (--)
// add per Event counters
//

//
static GLOBAL_UI_PENDING_INPUT_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn pending_input_event_inc(count: usize) -> usize {
    GLOBAL_UI_PENDING_INPUT_EVENT_COUNT.fetch_add(count, Ordering::SeqCst)
}

pub fn pending_input_event_dec(count: usize) -> usize {
    GLOBAL_UI_PENDING_INPUT_EVENT_COUNT.fetch_sub(count, Ordering::SeqCst)
}

pub fn pending_input_event_count() -> usize {
    GLOBAL_UI_PENDING_INPUT_EVENT_COUNT.load(Ordering::SeqCst)
}

//
static GLOBAL_UI_PENDING_RENDER_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn pending_render_event_inc(count: usize) -> usize {
    GLOBAL_UI_PENDING_RENDER_EVENT_COUNT.fetch_add(count, Ordering::SeqCst)
}

pub fn pending_render_event_dec(count: usize) -> usize {
    GLOBAL_UI_PENDING_RENDER_EVENT_COUNT.fetch_sub(count, Ordering::SeqCst)
}

pub fn pending_render_event_count() -> usize {
    GLOBAL_UI_PENDING_RENDER_EVENT_COUNT.load(Ordering::SeqCst)
}

/*
JSON based ?
{
// configuration

[
  {"ambiguous_exec_timeout": 250 },
  {"events": [ {"keys":   ["ctrl+c"] }, {"keys": ["ctrl+q"] }], "action": "application:quit" },
  {"events": [ {"keys":   ["ctrl+c"] } ],                       "action": "text-mode:copy" },
  {"events": [ {"keys":   ["ctrl+x"] } ],                       "action": "text-mode:cut" },
  {"events": [ {"keys":   ["ctrl+v"] } ],                       "action": "text-mode:paste" },
  {"events": [ {"system": [ "xxx" ] } ],                        "action": "text-mode:quit" },
  {"events": [ {"keys": ["a"] } ],                              "action": "text-mode:self-insert" },
  {"events": [ {"keys": { } ],                  "action": "self-insert" }, // default handler special syntax

  {"events": [ {"button_press": { "button":1} ],                 "action": "self-insert" }, // default handler special syntax



]

}
*/

/// Message sent between core and ui threads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventMessage {
    /// sequence number. should be reused in corresponding answer.
    pub seq: usize,
    /// underlying event.
    pub event: Event,
}

impl EventMessage {
    pub fn new(seq: usize, event: Event) -> Self {
        EventMessage { seq, event }
    }
}

/// Events sent between core and ui threads via EventMesssage encapsulation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    /// Sent by ui thread. Request the list of opened documents.
    RequestDocumentList,
    /// Sent by core thread. The list of opened documents.
    DocumentList {
        list: Vec<(document::Id, String)>,
    },

    /// Sent by ui thread. Request a view creation that maps the document referenced by doc_id.
    CreateView {
        width: usize,
        height: usize,
        doc_id: document::Id,
    },
    /// Sent by core thread. Answer to CreateView request.<br/>
    /// contains a unique view_id that MUST be reused with  other operations (DestroyView, ...).
    ViewCreated {
        width: usize,
        height: usize,
        doc_id: document::Id,
        view_id: view::Id,
    },

    /// Sent by ui thread. Request the destruction of a specific view referenced by view_id.
    DestroyView {
        width: usize,
        height: usize,
        doc_id: document::Id,
        view_id: view::Id,
    },
    /// Sent by core thread. Answer to DestroyView request.<br/>
    ViewDestroyed {
        width: usize,
        height: usize,
        doc_id: document::Id,
        view_id: view::Id,
    },
    /// Sent by ui thread. contains user input information.
    InputEvent {
        events: Vec<self::InputEvent>,
        raw_data: Option<Vec<u8>>, /* raw data for debug */
    },
    /// Sent by ui thread. Request the rendering of a given view.
    RequestLayoutEvent {
        view_id: view::Id,
        doc_id: document::Id,
        width: usize,  // used to detect change
        height: usize, // used to detect change
    },
    /// Sent by core thread. Contains the rendered screen that maps view_id.
    BuildLayoutEvent {
        view_id: view::Id,
        doc_id: document::Id,
        screen: Box<Screen>,
    },
    /// for future version, will map operating system events.
    SystemEvent,

    /// for future version, will map operating system ui events (minimize, close, ...).
    ApplicationEvent,

    /// Sent by ui thread. Request to resize a given view referenced by view_id.
    ResizeEvent {
        view_id: view::Id,
        width: usize,
        height: usize,
    },
    ApplicationQuitEvent,
}

#[derive(Hash, Debug, Clone, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl KeyModifiers {
    pub fn new() -> KeyModifiers {
        KeyModifiers {
            ctrl: false,
            alt: false,
            shift: false,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ButtonEvent {
    pub button: u32,
    pub x: i32,
    pub y: i32,
    pub mods: KeyModifiers,
}

impl Hash for ButtonEvent {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.button.hash(state);
        // ignore (x, y) coordinates
        self.mods.hash(state);
    }
}

#[derive(Hash, Debug, Clone, PartialEq, Eq)]
pub struct PointerEvent {
    pub x: i32,
    pub y: i32,
    pub mods: KeyModifiers,
}

impl PointerEvent {
    pub fn hash(&mut self) -> u64 {
        let mut s = DefaultHasher::new();
        // ignore x, y
        self.x.hash(&mut s);
        self.y.hash(&mut s);
        self.mods.hash(&mut s);
        s.finish()
    }
}

// TODO: special hash for ButtonPress/ButtonRelease that ignores (x,y)

/// Supported input events
#[derive(Hash, Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    InvalidInputEvent,
    NoInputEvent,
    FallbackEvent, // use to map default action in input table
    KeyPress { key: Key, mods: KeyModifiers },
    ButtonPress(ButtonEvent),
    ButtonRelease(ButtonEvent),
    PointerMotion(PointerEvent),
    WheelUp { mods: KeyModifiers, x: i32, y: i32 },
    WheelDown { mods: KeyModifiers, x: i32, y: i32 },
}

/// List of supported keyboard keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    NUL,
    Unicode(char), // utf32 codepoints
    UnicodeArray(Vec<char>),
    Clear,
    Pause,
    ScrollLock,
    SysReq,
    Escape,
    Delete,
    BackSpace,
    Insert,
    Home,
    Left,
    Up,
    Right,
    Down,
    PageUp,
    PageDown,
    End,
    Begin,
    F(usize),
    ///
    KeypadPlus,
    KeypadMinus,
    KeypadMul,
    KeypadDiv,
    KeypadEnter,
    NoKey,
}

pub type InputEventHash = u64;
pub type InputEventMap = HashMap<InputEventHash, Rc<InputEventRule>>;

#[derive(Debug)]
pub struct InputEventRule {
    // range ?
    pub action: Option<String>,
    pub children: Option<Rc<RefCell<InputEventMap>>>,
}

/// This function is the central point where
/// we can identify event of the same class
/// example:
/// for ButtonEvent we do partial hasing , ie: we ignore the coordinates of the pointer
/// for other events we relry on #[derive(Hash)]
fn compute_input_event_hash(t: &InputEvent) -> InputEventHash {
    let mut s = DefaultHasher::new();

    match t {
        InputEvent::ButtonPress(ref button_event) => match button_event {
            ButtonEvent {
                mods,
                x: _,
                y: _,
                button,
            } => {
                "ButtonPress".hash(&mut s);
                (*button).hash(&mut s);
                // ignore x y
                // (*x).hash(&mut s);
                // (*y).hash(&mut s);
                (*mods).hash(&mut s)
            }
        },

        InputEvent::ButtonRelease(ref button_event) => match button_event {
            ButtonEvent {
                mods,
                x: _,
                y: _,
                button,
            } => {
                "ButtonRelease".hash(&mut s);
                (*button).hash(&mut s);
                // ignore x y
                // (*x).hash(&mut s);
                // (*y).hash(&mut s);
                (*mods).hash(&mut s)
            }
        },

        InputEvent::WheelUp { mods, x, y } => {
            "WheelUp".hash(&mut s);
            // ignore x y
            // (*x).hash(&mut s);
            // (*y).hash(&mut s);
            (*mods).hash(&mut s)
        }

        InputEvent::WheelDown { mods, x, y } => {
            "WheelDown".hash(&mut s);
            // ignore x y
            // (*x).hash(&mut s);
            // (*y).hash(&mut s);
            (*mods).hash(&mut s)
        }

        _ => t.hash(&mut s),
    }

    s.finish()
}
