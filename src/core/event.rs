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

use std::cell::RefCell;
use std::mem;
use std::rc::Rc;

use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};

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

#[derive(Hash, Debug, Clone, PartialEq, Eq)]
pub struct KeyModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
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
    WheelUp { mods: KeyModifiers },
    WheelDown { mods: KeyModifiers },
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

type InputEventHash = u64;
type InputEventMap = HashMap<InputEventHash, Rc<InputEventRule>>;

#[derive(Debug)]
struct InputEventRule {
    // range ?
    pub action: Option<String>,
    pub children: Option<Rc<RefCell<InputEventMap>>>,
}

// intermediate hash as key ?
fn input_event_rule_hash(t: &InputEvent) -> InputEventHash {
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

        _ => t.hash(&mut s),
    }

    s.finish()
}

// cargo test -- --nocapture test_input_map

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_input_map() {
        let mut h: InputEventMap = HashMap::new();

        //
        let event = InputEvent::KeyPress {
            key: Key::Left,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };

        let val = input_event_rule_hash(&event);

        println!("val = {:?}", val);

        h.insert(
            input_event_rule_hash(&event),
            Rc::new(InputEventRule {
                action: Some("move-forward".to_string()),
                children: None,
            }),
        );

        let event_user = InputEvent::KeyPress {
            key: Key::Left,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };

        let val = input_event_rule_hash(&event_user);

        let value = h.get(&val);

        println!("{:?}", value);

        let button_ref_event = InputEvent::ButtonPress(ButtonEvent {
            button: 0,
            x: 0,
            y: 0,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        });

        h.insert(
            input_event_rule_hash(&button_ref_event),
            Rc::new(InputEventRule {
                action: Some("begin-selection".to_string()),
                children: None,
            }),
        );

        let button_event_user = InputEvent::ButtonPress(ButtonEvent {
            button: 0,
            x: 123,
            y: 0,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        });

        let val = input_event_rule_hash(&button_event_user);

        let button_value = h.get(&val);

        let button_event_hash = input_event_rule_hash(&button_ref_event);
        let button_event_user_hash = input_event_rule_hash(&button_event_user);

        println!("button_event_hash      = {:?}", button_event_hash);
        println!("button_event_user_hash = {:?}", button_event_user_hash);

        println!("{:?}", button_value);
        println!(
            "button_ref_event == button_event_user -> {:?}",
            button_ref_event == button_event_user
        );
    }

    #[test]
    fn build_input_map() -> Result<(), serde_json::error::Error> {
        struct ParseCtx {
            action: String,
            is_default: bool,
            sequence: Vec<InputEvent>,
            map: Rc<RefCell<InputEventMap>>,
        }

        impl ParseCtx {
            fn new() -> ParseCtx {
                ParseCtx {
                    action: String::new(),
                    is_default: false,
                    sequence: Vec::new(),
                    map: Rc::new(RefCell::new(InputEventMap::new())),
                }
            }

            fn build_map_entry(&mut self) {
                println!("building entry for '{}'", self.action);

                // TODO: user iter instead of index
                fn read_sequence(
                    is_default: bool,
                    map: &mut InputEventMap,
                    sequence: &Vec<InputEvent>,
                    pos: usize,
                    action: &String,
                ) {
                    if pos == sequence.len() {
                        if is_default {
                            // TODO: check action
                            let ev = InputEvent::FallbackEvent;
                            let event_hash = input_event_rule_hash(&ev);
                            // TODO: replace
                            map.remove(&event_hash);
                            map.entry(event_hash).or_insert(Rc::new(InputEventRule {
                                action: Some(action.clone()),
                                children: None,
                            }));
                        }

                        return;
                    }

                    let e = &sequence[pos];
                    let event_hash = input_event_rule_hash(&e);

                    let rule = &mut map.entry(event_hash).or_insert(Rc::new(InputEventRule {
                        action: if pos + 1 == sequence.len() {
                            Some(action.clone())
                        } else {
                            None
                        },
                        children: if pos + 1 == sequence.len() {
                            None
                        } else {
                            Some(Rc::new(RefCell::new(HashMap::new())))
                        },
                    }));

                    //                    println!("rule = {:?}", rule);

                    if pos + 1 == sequence.len() {
                        return;
                    }

                    if let Some(ref mut map) = rule.children.as_ref() {
                        read_sequence(
                            is_default,
                            &mut map.as_ref().borrow_mut(),
                            sequence,
                            pos + 1,
                            &action,
                        );
                    }
                }

                let map = &mut self.map.as_ref().borrow_mut();
                read_sequence(self.is_default, map, &self.sequence, 0, &self.action);

                //
                // TODO: self.reset();
                self.action.clear();
                self.sequence.clear();
                self.is_default = false;
            }
        }

        let mut ctx = ParseCtx::new();

        use serde_json::Value;

        let data = r#"[{
             "events": [
                { "in": [{ "key": "Left"     }],                        "action": "text-mode:move-mark-backward" },
                { "in": [{ "key": "Right"    }],                        "action": "text-mode:move-mark-forward" },
                { "in": [{ "key": "Up"       }],                        "action": "text-mode:move-mark-to-previous-line" },
                { "in": [{ "key": "Down"     }],                        "action": "text-mode:move-mark-to-next-line" },
                { "in": [{ "key": "PageUp"   }],                        "action": "text-mode:move-to-previous-screen" },
                { "in": [{ "key": "PageDown" }],                        "action": "text-mode:move-to-next-screen" },

                { "in": [{ "key": "ctrl+alt+Left"     }],                "action": "text-mode:move-mark-backward-word" },
                { "in": [{ "key": "ctrl+alt+Right"     }],                "action": "text-mode:move-mark-one-forward" },

                { "in": [{ "key": "ctrl+€"      }],                     "action": "" },

                { "in": [{ "key": "Esc"      }],                        "action": "editor:cancel" },
                { "in": [{ "key": "ctrl+g"   }],                        "action": "editor:cancel" },

                { "in": [{ "key": "ctrl+q"   }],                        "action": "application:quit" },

                { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+c" } ],    "action": "application:quit" },

                { "in": [{ "key": "ctrl+x" }, { "key": "ctrl+b" } ],    "action": "application:quit2" },

                { "in": [{ "system": "SIGTERM" } ],                      "action": "application:quit" },

                { "default": [], "action": "text-mode:self-insert" }
              ]
        }]"#;

        // Parse the string of data into serde_json::Value.
        let json: Value = serde_json::from_str(data)?;
        println!("parsing {:?}", json);

        /*
         enum Value {
            Null,
            Bool(bool),
            Number(Number),
            String(String),
            Array(Vec<Value>),
            Object(Map<String, Value>),
        }
        */
        let vec = if let Value::Array(ref vec) = json {
            vec
        } else {
            return Ok(());
        };

        // parse 1st level entries
        for obj in vec {
            println!("obj = {:?}", obj);
            if let Value::Object(map) = obj {
                for (k, v) in map {
                    println!("k = {:?}", k);
                    match k.as_str() {
                        "events" => {
                            parse_event_entry(&mut ctx, k, v);
                        }
                        _ => {}
                    }
                }
            }
        }

        fn parse_event_entry(mut ctx: &mut ParseCtx, name: &String, value: &serde_json::Value) {
            println!("fount event '{}'", name);
            let vec = if let Value::Array(ref vec) = value {
                vec
            } else {
                // parse error
                return;
            };

            for obj in vec {
                // println!("obj = {:?}", obj);
                if let Value::Object(map) = obj {
                    println!("---------- new entry");
                    for (k, v) in map {
                        println!("k = {:?}", k);
                        match k.as_str() {
                            "in" => {
                                parse_event_entry_input(&mut ctx, k, v);
                            }
                            "action" => {
                                parse_event_entry_action(&mut ctx, k, v);
                            }
                            "default" => {
                                parse_event_entry_default_action(&mut ctx, k, v);
                            }

                            _ => {}
                        }
                    }
                    ctx.build_map_entry();
                }
            }
        }

        fn parse_event_entry_action(
            mut ctx: &mut ParseCtx,
            name: &String,
            value: &serde_json::Value,
        ) {
            // copy string to event
            if let Value::String(ref s) = value {
                println!("action = '{}'", s);
                ctx.action = s.clone();
            }
        }

        fn parse_event_entry_default_action(
            mut ctx: &mut ParseCtx,
            name: &String,
            value: &serde_json::Value,
        ) {
            println!("parse_event_entry_default_action = '{}'", value);
            ctx.is_default = true;
        }

        fn parse_event_entry_input(
            mut ctx: &mut ParseCtx,
            name: &String,
            value: &serde_json::Value,
        ) {
            let vec = if let Value::Array(ref vec) = value {
                vec
            } else {
                // parse error
                return;
            };

            for obj in vec {
                //println!("obj = {:?}", obj);
                if let Value::Object(map) = obj {
                    for (k, v) in map {
                        //println!("k = {:?}", k);
                        match k.as_str() {
                            "key" => {
                                parse_event_entry_input_key(&mut ctx, k, v);
                            }
                            "click" => {
                                parse_event_entry_input_click(&mut ctx, k, v);
                            }

                            _ => {}
                        }
                    }
                }
            }
        }

        fn parse_event_entry_input_key(
            ctx: &mut ParseCtx,
            name: &String,
            value: &serde_json::Value,
        ) {
            let s = if let Value::String(ref s) = value {
                println!("value = '{}'", s);
                s
            } else {
                // syntax error
                return;
            };

            // parse "key" value ctrl+alt+shift+x
            println!("{{");

            let mut mods = KeyModifiers {
                ctrl: false,
                alt: false,
                shift: false,
            };

            let mut key = Key::NoKey;

            for k in s.split("+") {
                println!("key = {:?}", k);
                match k {
                    "ctrl" => mods.ctrl = true,
                    "alt" => mods.alt = true,
                    "shift" => mods.shift = true,
                    "Clear" => key = Key::Clear,
                    "Pause" => key = Key::Pause,
                    "ScrollLock" => key = Key::ScrollLock,
                    "SysReq" => key = Key::SysReq,
                    "Escape" => key = Key::Escape,
                    "Delete" => key = Key::Delete,
                    "BackSpace" => key = Key::BackSpace,
                    "Insert" => key = Key::Insert,
                    "Home" => key = Key::Home,
                    "Left" => key = Key::Left,
                    "Up" => key = Key::Up,
                    "Right" => key = Key::Right,
                    "Down" => key = Key::Down,
                    "PageUp" => key = Key::PageUp,
                    "PageDown" => key = Key::PageDown,
                    "End" => key = Key::End,
                    "Begin" => key = Key::Begin,
                    "F1" => key = Key::F(1),
                    "F2" => key = Key::F(2),
                    "F3" => key = Key::F(3),
                    "F4" => key = Key::F(4),
                    "F5" => key = Key::F(5),
                    "F6" => key = Key::F(6),
                    "F7" => key = Key::F(7),
                    "F8" => key = Key::F(8),
                    "F9" => key = Key::F(9),
                    "F10" => key = Key::F(10),
                    "F11" => key = Key::F(11),
                    "F12" => key = Key::F(12),
                    "KeypadPlus" => key = Key::KeypadPlus,
                    "KeypadMinus" => key = Key::KeypadMinus,
                    "KeypadMul" => key = Key::KeypadMul,
                    "KeypadDiv" => key = Key::KeypadDiv,
                    "KeypadEnter" => key = Key::KeypadEnter,
                    _ => {
                        if let Some(c) = k.chars().nth(0) {
                            key = Key::Unicode(c);
                        }
                    }
                }
            }

            println!("}}");

            let ev = InputEvent::KeyPress { key, mods };

            println!("built event = {:?}", ev);

            ctx.sequence.push(ev)
        }

        fn parse_event_entry_input_click(
            ctx: &mut ParseCtx,
            name: &String,
            value: &serde_json::Value,
        ) {
            if let Value::String(ref s) = value {
                println!("button = '{}'", s);
            }
        }

        //        let mut hi: HashMap<u64, Box<InputEventRule>> = HashMap::new();
        println!("****** print map");
        for (k, v) in ctx.map.as_ref().borrow().iter() {
            println!("{:?} -> {:?}", k, v);
        }

        let mut iev = Vec::new();

        iev.push(InputEvent::KeyPress {
            key: Key::Unicode('€'),
            mods: KeyModifiers {
                ctrl: false,
                alt: false,
                shift: false,
            },
        });

        /*
                iev.push(InputEvent::KeyPress {
                    key: Key::Unicode('x'),
                    mods: KeyModifiers {
                        ctrl: true,
                        alt: false,
                        shift: false,
                    },
                });
                iev.push(InputEvent::KeyPress {
                    key: Key::Unicode('c'),
                    mods: KeyModifiers {
                        ctrl: true,
                        alt: false,
                        shift: false,
                    },
                });
        */

        fn eval_input_event(
            ev: &InputEvent,
            input_map: &Rc<RefCell<InputEventMap>>,
            in_node: &mut Option<Rc<InputEventRule>>,
            out_node: &mut Option<Rc<InputEventRule>>,
        ) -> Option<String> {
            println!("\n\n eval_input_event");

            println!("found in_node {:?}", in_node);

            let event_hash = input_event_rule_hash(ev);
            println!("event_hash = {}", event_hash);

            // not first level ?
            if let Some(node) = in_node.as_ref() {
                if let Some(map) = &node.as_ref().children {
                    let map = map.as_ref().borrow();
                    match map.get(&event_hash) {
                        Some(event) => {
                            if let Some(action) = &event.as_ref().action {
                                println!("\n\n eval_input_event");
                                return Some(action.to_string());
                            }

                            *out_node = Some(Rc::clone(event));

                            println!("found out_node {:?}", out_node);
                        }
                        None => {}
                    }
                }
            } else {
                match input_map.as_ref().borrow().get(&event_hash) {
                    Some(event) => {
                        if let Some(action) = &event.as_ref().action {
                            println!("found action");
                            return Some(action.to_string());
                        }

                        *out_node = Some(Rc::clone(event));

                        println!("found out_node {:?}", out_node);
                    }
                    None => {
                        println!("TODO: look for default action");
                        let ev = InputEvent::FallbackEvent;
                        let event_hash = input_event_rule_hash(&ev);

                        match input_map.as_ref().borrow().get(&event_hash) {
                            Some(event) => {
                                if let Some(action) = &event.as_ref().action {
                                    println!("default found action {}", action);
                                    return Some(action.to_string());
                                }

                                *out_node = None;
                                println!("cancel sequence");
                            }
                            None => {
                                println!("no default action defined");
                                *out_node = None;
                            }
                        }
                    }
                }
            };

            None
        }

        let rc_map = Rc::new(ctx.map);

        let mut current_node: Option<Rc<InputEventRule>> = None;
        let mut next_node: Option<Rc<InputEventRule>> = None;

        for ev in &iev {
            let action = eval_input_event(&ev, &rc_map, &mut current_node, &mut next_node);
            if let Some(action) = action {
                println!("found action {}", action);
            } else {
                std::mem::swap(&mut current_node, &mut next_node);
            }
        }

        Ok(())
    }
}
