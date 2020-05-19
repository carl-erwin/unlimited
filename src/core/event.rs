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

use crate::core::document;
use crate::core::screen::Screen;
use crate::core::view;

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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

/// Supported input events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    InvalidInputEvent,
    NoInputEvent,
    KeyPress {
        key: Key,
        mods: KeyModifiers,
    },
    ButtonPress {
        button: u32,
        x: i32,
        y: i32,
        mods: KeyModifiers,
    },
    ButtonRelease {
        button: u32,
        x: i32,
        y: i32,
        mods: KeyModifiers,
    },
    PointerMotion {
        x: i32,
        y: i32,
        mods: KeyModifiers,
    },
    WheelUp {
        mods: KeyModifiers,
    },
    WheelDown {
        mods: KeyModifiers,
    },
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

/*

#[test]
fn test_event_serde() {
    let event = Event::InputEvent {
        events: vec![InputEvent::KeyPress {
            key: Key::Left,
            mods: KeyModifiers {
            ctrl: false,
            shift: false,
            alt: false,
        },
        }],
        raw_data: None,
    };

    // Convert the Point to a JSON string.
    let serialized = serde_json::to_string(&event).unwrap();

    // Prints serialized = {"x":1,"y":2}
    println!("serialized = {}", serialized);

    // Convert the JSON string back to a Point.
    let deserialized: Event = serde_json::from_str(&serialized).unwrap();

    // Prints deserialized = Point { x: 1, y: 2 }
    println!("deserialized = {:?}", deserialized);
}

*/
