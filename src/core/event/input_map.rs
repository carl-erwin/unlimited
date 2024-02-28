// cargo test -- --nocapture test_input_map

// cargo test -- --nocapture build_input_map

use serde_json::Value;

use super::*;

pub static DEFAULT_INPUT_MAP: &str = std::include_str!("../../../res/main-input-map.json");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DefaultActionMode {
    IgnoreDefaultAction,
    RunDefaultAction,
}

// TODO(ceg): map error to editor error
// unlimited::error::SyntaxError(file, line, col, str_details);
pub fn build_input_event_map(json: &str) -> Result<InputEventMap, serde_json::error::Error> {
    let mut ctx = ParseCtx::new();

    // Parse the string of data into serde_json::Value.
    let json: Value = serde_json::from_str(json)?;
    //dbg_println!("parsing {:?}", json);

    let vec = if let Value::Array(ref vec) = json {
        vec
    } else {
        return Ok(InputEventMap::new());
    };

    // parse 1st level entries
    for obj in vec {
        //dbg_println!("obj = {:?}", obj);
        if let Value::Object(map) = obj {
            for (k, v) in map {
                //dbg_println!("k = {:?}", k);
                match k.as_str() {
                    "events" => {
                        parse_event_entry(&mut ctx, k, v);
                    }
                    _ => {}
                }
            }
        }
    }

    Ok(ctx.map)
}

struct ParseCtx {
    action: String,
    is_default: bool,
    sequence: Vec<InputEvent>,
    map: InputEventMap,
}

impl ParseCtx {
    fn new() -> ParseCtx {
        ParseCtx {
            action: String::new(),
            is_default: false,
            sequence: Vec::new(),
            map: InputEventMap::new(),
        }
    }

    fn build_map_entry(&mut self) {
        //dbg_println!("building entry for '{}'", self.action);

        // TODO(ceg): user iter instead of index
        fn read_sequence(
            is_default: bool,
            map: &mut InputEventMap,
            sequence: &Vec<InputEvent>,
            pos: usize,
            action: &String,
        ) {
            if pos == sequence.len() {
                if is_default {
                    // TODO(ceg): check action
                    let ev = InputEvent::FallbackEvent;
                    let event_hash = compute_input_event_hash(&ev);
                    // TODO(ceg): replace
                    map.remove(&event_hash);
                    map.entry(event_hash).or_insert(Rc::new(InputEventRule {
                        action: Some(action.clone()),
                        children: None,
                    }));
                }

                return;
            }

            let e = &sequence[pos];
            let event_hash = compute_input_event_hash(&e);

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

            //                    //dbg_println!("rule = {:?}", rule);

            if pos + 1 == sequence.len() {
                return;
            }

            if let Some(ref mut map) = rule.children.as_ref() {
                read_sequence(
                    is_default,
                    &mut map.borrow_mut(),
                    sequence,
                    pos + 1,
                    &action,
                );
            }
        }

        let map = &mut self.map;
        read_sequence(self.is_default, map, &self.sequence, 0, &self.action);

        //
        // TODO(ceg): self.reset();
        self.action.clear();
        self.sequence.clear();
        self.is_default = false;
    }
}

fn parse_event_entry_input_key(ctx: &mut ParseCtx, _name: &String, value: &serde_json::Value) {
    let s = if let Value::String(ref s) = value {
        // //dbg_println!("value = '{}'", s);
        s
    } else {
        // syntax error
        return;
    };

    // parse "key" value ctrl+alt+shift+x
    // //dbg_println!("{{");

    let mut mods = KeyModifiers {
        ctrl: false,
        alt: false,
        shift: false,
    };

    let mut key = Key::NoKey;

    for k in s.split("+") {
        // //dbg_println!("key = {:?}", k);
        // match to_lower(k).as_str() ?
        match k {
            "ctrl" => mods.ctrl = true,
            "alt" => mods.alt = true,
            "shift" => mods.shift = true,
            "Clear" => key = Key::Clear,
            "Pause" => key = Key::Pause,
            "ScrollLock" => key = Key::ScrollLock,
            "SysReq" => key = Key::SysReq,
            "Esc" => key = Key::Escape,
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
            "Space" => key = Key::Unicode(' '),
            "Tab" => key = Key::Unicode('\t'),
            _ => {
                if let Some(c) = k.chars().nth(0) {
                    key = Key::Unicode(c);
                }
            }
        }
    }

    // //dbg_println!("}}");

    let ev = InputEvent::KeyPress { key, mods };

    // //dbg_println!("built event = {:?}", ev);

    ctx.sequence.push(ev)
}

fn parse_event_entry_input_button_press(
    ctx: &mut ParseCtx,
    _name: &String,
    value: &serde_json::Value,
) {
    let s = if let Value::String(ref s) = value {
        // //dbg_println!("value = '{}'", s);
        s
    } else {
        // syntax error
        return;
    };

    // parse "key" value 0
    //dbg_println!("{{");

    let mods = KeyModifiers::new();
    //dbg_println!("button = {:?}", s);
    let button: u32 = match s.as_str() {
        "0" => 0,
        "1" => 1,
        "2" => 2,
        _ => {
            return;
        }
    };

    //dbg_println!("}}");

    let ev = InputEvent::ButtonPress(ButtonEvent {
        button,
        x: 0,
        y: 0,
        mods,
    });

    //dbg_println!("built button event = {:?}", ev);

    ctx.sequence.push(ev)
}

// TODO(ceg): refactor with  parse_event_entry_input_button_press
fn parse_event_entry_input_button_release(
    ctx: &mut ParseCtx,
    _name: &String,
    value: &serde_json::Value,
) {
    let s = if let Value::String(ref s) = value {
        //dbg_println!("value = '{}'", s);
        s
    } else {
        // syntax error
        return;
    };

    // parse "key" value 0
    //dbg_println!("{{");

    let mods = KeyModifiers::new();
    //dbg_println!("button = {:?}", s);
    let button: u32 = match s.as_str() {
        "0" => 0,
        "1" => 1,
        "2" => 2,
        _ => {
            return;
        }
    };

    //dbg_println!("}}");

    let ev = InputEvent::ButtonRelease(ButtonEvent {
        button,
        x: 0,
        y: 0,
        mods,
    });

    //dbg_println!("built button event = {:?}", ev);

    ctx.sequence.push(ev)
}

fn parse_event_entry_input_wheel(ctx: &mut ParseCtx, _name: &String, value: &serde_json::Value) {
    let s = if let Value::String(ref s) = value {
        //dbg_println!("value = '{}'", s);
        s
    } else {
        // syntax error
        return;
    };

    // parse "key" value 0
    //dbg_println!("{{");

    let mods = KeyModifiers::new();
    //dbg_println!("button = {:?}", s);

    let ev = match s.as_str() {
        "Up" => InputEvent::WheelUp { x: 0, y: 0, mods },
        "Down" => InputEvent::WheelDown { x: 0, y: 0, mods },
        _ => {
            return;
        }
    };

    //dbg_println!("}}");

    //dbg_println!("building wheel event = {:?}", ev);

    ctx.sequence.push(ev)
}

fn parse_event_entry_input_pointer_motion(
    ctx: &mut ParseCtx,
    _name: &String,
    value: &serde_json::Value,
) {
    let _s = if let Value::String(ref s) = value {
        //dbg_println!("value = '{}'", s);
        s
    } else {
        // syntax error
        return;
    };

    // parse "key" value 0
    //dbg_println!("{{");

    let mods = KeyModifiers::new();

    let ev = InputEvent::PointerMotion(PointerEvent { x: 0, y: 0, mods });

    //dbg_println!("}}");

    //dbg_println!("building pointer motion event = {:?}", ev);

    ctx.sequence.push(ev)
}

fn parse_event_entry(mut ctx: &mut ParseCtx, _name: &String, value: &serde_json::Value) {
    //dbg_println!("fount event '{}'", name);
    let vec = if let Value::Array(ref vec) = value {
        vec
    } else {
        // parse error
        return;
    };

    for obj in vec {
        // //dbg_println!("obj = {:?}", obj);
        if let Value::Object(map) = obj {
            // //dbg_println!("---------- new entry");
            for (k, v) in map {
                // //dbg_println!("k = {:?}", k);
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

fn parse_event_entry_action(ctx: &mut ParseCtx, _name: &String, value: &serde_json::Value) {
    // copy string to event
    if let Value::String(ref s) = value {
        //dbg_println!("action = '{}'", s);
        ctx.action = s.clone();
    }
}

fn parse_event_entry_default_action(
    ctx: &mut ParseCtx,
    _name: &String,
    _value: &serde_json::Value,
) {
    // //dbg_println!("parse_event_entry_default_action = '{}'", value);
    ctx.is_default = true;
}

fn parse_event_entry_input(mut ctx: &mut ParseCtx, _name: &String, value: &serde_json::Value) {
    let vec = if let Value::Array(ref vec) = value {
        vec
    } else {
        // parse error
        return;
    };

    for obj in vec {
        //dbg_println!("obj = {:?}", obj);
        if let Value::Object(map) = obj {
            for (k, v) in map {
                //dbg_println!("k = {:?}", k);
                match k.as_str() {
                    "key" => {
                        parse_event_entry_input_key(&mut ctx, k, v);
                    }
                    "button-press" => {
                        parse_event_entry_input_button_press(&mut ctx, k, v);
                    }
                    "button-release" => {
                        parse_event_entry_input_button_release(&mut ctx, k, v);
                    }
                    "wheel" => {
                        parse_event_entry_input_wheel(&mut ctx, k, v);
                    }
                    "pointer-motion" => {
                        parse_event_entry_input_pointer_motion(&mut ctx, k, v);
                    }

                    //                    "button-drag" => {
                    //                        parse_event_entry_input_button_drag(&mut ctx, k, v);
                    //                    }
                    _ => {}
                }
            }
        }
    }
}

fn reset_io_nodes(
    in_node: &mut Option<Rc<InputEventRule>>,
    out_node: &mut Option<Rc<InputEventRule>>,
) {
    *in_node = None;
    *out_node = None;
}

fn walk_input_event_tree(
    event_hash: u64,
    node: &Rc<InputEventRule>,
    mut in_node: &mut Option<Rc<InputEventRule>>,
    mut out_node: &mut Option<Rc<InputEventRule>>,
) -> Option<String> {
    if let Some(map) = &node.as_ref().children {
        let map = map.borrow();
        match map.get(&event_hash) {
            Some(event) => {
                if let Some(action) = &event.as_ref().action {
                    return Some(action.to_string());
                }

                *out_node = Some(Rc::clone(event));
                dbg_println!("found out_node {:?}", out_node);
            }
            None => {}
        }
    } else {
        // dbg_println!("no children found: reset");
        reset_io_nodes(&mut in_node, &mut out_node);
    }
    None
}

pub fn eval_input_event(
    ev: &InputEvent,
    input_map: &InputEventMap,
    default_action_mode: DefaultActionMode,
    mut in_node: &mut Option<Rc<InputEventRule>>,
    mut out_node: &mut Option<Rc<InputEventRule>>,
) -> Option<String> {
    dbg_println!("   eval_input_event --------------------------");

    // dbg_println!("input map  {:?}", input_map);

    let event_hash = compute_input_event_hash(ev);

    dbg_println!("ev = {:?}", *ev);
    dbg_println!("event_hash = {}", event_hash);

    if let Some(node) = in_node.clone() {
        return walk_input_event_tree(event_hash, &node, in_node, out_node);
    }

    // NB: fallback happens only at first level
    dbg_println!("--- 1st level event ---");

    match input_map.get(&event_hash) {
        Some(event) => {
            if let Some(action) = &event.as_ref().action {
                reset_io_nodes(&mut in_node, &mut out_node);
                return Some(action.to_string());
            }

            *out_node = Some(Rc::clone(event));
        }
        None => {
            reset_io_nodes(&mut in_node, &mut out_node);
            if default_action_mode == DefaultActionMode::IgnoreDefaultAction {
                return None;
            };

            // get fallback action
            let event_hash = compute_input_event_hash(&InputEvent::FallbackEvent);
            match input_map.get(&event_hash) {
                Some(event) => {
                    if let Some(action) = &event.as_ref().action {
                        return Some(action.to_string());
                    }
                }
                None => {
                    dbg_println!("no default action defined");
                }
            }
        }
    }

    None
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn test_input_map() {
        //
        let keypress_event = InputEvent::KeyPress {
            key: Key::Left,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };

        let _keypress_event_hash = compute_input_event_hash(&keypress_event);

        //dbg_println!("keypress_event hash = {:?}", keypress_event_hash);

        let mut h: InputEventMap = HashMap::new();
        h.insert(
            compute_input_event_hash(&keypress_event),
            Rc::new(InputEventRule {
                action: Some("move-forward".to_string()),
                children: None,
            }),
        );

        let keypress_event = InputEvent::KeyPress {
            key: Key::Left,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        };

        let keypress_event_hash = compute_input_event_hash(&keypress_event);

        let _rule = h.get(&keypress_event_hash);

        //dbg_println!("{:?}", rule);

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
            compute_input_event_hash(&button_ref_event),
            Rc::new(InputEventRule {
                action: Some("begin-selection".to_string()),
                children: None,
            }),
        );

        let button_event = InputEvent::ButtonPress(ButtonEvent {
            button: 0,
            x: 123,
            y: 0,
            mods: KeyModifiers {
                ctrl: false,
                shift: false,
                alt: false,
            },
        });

        let val = compute_input_event_hash(&button_event);

        let _button_value = h.get(&val);

        let button_ref_event_hash = compute_input_event_hash(&button_ref_event);
        let button_event_hash = compute_input_event_hash(&button_event);

        //dbg_println!("button_ref_event_hash      = {:?}", button_ref_event_hash);
        //dbg_println!("button_event_hash = {:?}", button_event_hash);

        //dbg_println!("{:?}", button_value);
        //dbg_println!(
        //    "button_ref_event == button_event_user -> {:?}",
        //    button_ref_event == button_event
        //);

        assert_eq!(button_ref_event_hash, button_event_hash);
    }

    #[test]

    fn test_build_input_event_map() -> Result<(), serde_json::error::Error> {
        let map = build_input_event_map(DEFAULT_INPUT_MAP)?;

        //dbg_println!("****** print map");
        for (_k, _v) in map.iter() {
            //dbg_println!("{:?} -> {:?}", k, v);
        }

        let mut iev = Vec::new();

        // test eval
        {
            iev.push(InputEvent::KeyPress {
                key: Key::Unicode('x'),
                mods: KeyModifiers {
                    ctrl: true,
                    alt: false,
                    shift: false,
                },
            });
            iev.push(InputEvent::KeyPress {
                key: Key::Unicode('3'),
                mods: KeyModifiers {
                    ctrl: false,
                    alt: false,
                    shift: false,
                },
            });

            let mut current_node: Option<Rc<InputEventRule>> = None;
            let mut next_node: Option<Rc<InputEventRule>> = None;

            for ev in &iev {
                let action = eval_input_event(
                    &ev,
                    &map,
                    DefaultActionMode::RunDefaultAction,
                    &mut current_node,
                    &mut next_node,
                );
                if let Some(_action) = action {
                    //dbg_println!("found action {}", action);
                } else {
                    std::mem::swap(&mut current_node, &mut next_node);
                }
            }
        }

        Ok(())
    }
}
