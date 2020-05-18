// module export
pub mod input_map;

use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::sync::RwLock;
use std::time::Instant;
use std::vec::Vec;

use crate::core::document;
use crate::core::document::Document;
use crate::core::screen::Screen;

//
// TODO: implement functions ti update the counters
// on send (++) / receive (--)
// add per Event counters
//

//
static UI_PENDING_INPUT_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn pending_input_event_inc(count: usize) -> usize {
    UI_PENDING_INPUT_EVENT_COUNT.fetch_add(count, Ordering::SeqCst)
}

pub fn pending_input_event_dec(count: usize) -> usize {
    UI_PENDING_INPUT_EVENT_COUNT.fetch_sub(count, Ordering::SeqCst)
}

pub fn pending_input_event_count() -> usize {
    UI_PENDING_INPUT_EVENT_COUNT.load(Ordering::SeqCst)
}

//
static UI_PENDING_RENDER_EVENT_COUNT: AtomicUsize = AtomicUsize::new(0);

pub fn pending_render_event_inc(count: usize) -> usize {
    UI_PENDING_RENDER_EVENT_COUNT.fetch_add(count, Ordering::SeqCst)
}

pub fn pending_render_event_dec(count: usize) -> usize {
    UI_PENDING_RENDER_EVENT_COUNT.fetch_sub(count, Ordering::SeqCst)
}

pub fn pending_render_event_count() -> usize {
    UI_PENDING_RENDER_EVENT_COUNT.load(Ordering::SeqCst)
}

/// Message sent between core and ui threads.
#[derive(Debug, Clone)]
pub struct EventMessage<'a> {
    /// sequence number. should be reused in corresponding answer.
    pub seq: usize,
    /// underlying event.
    pub event: Event<'a>,
    // pub reply_to: Sender<EventMessage>, // clone
}

impl<'a> EventMessage<'a> {
    pub fn new(seq: usize, event: Event<'a>) -> Self {
        EventMessage { seq, event }
    }
}

/// Events sent between core and ui threads via EventMesssage encapsulation.
#[derive(Debug, Clone)]
pub enum Event<'a> {
    /// Sent by ui thread. Request the rendering of a given view.
    UpdateViewEvent {
        width: usize,  // used to detect change
        height: usize, // used to detect change
    },

    /// Sent by core thread. Contains the rendered screen that maps view_id.
    DrawEvent {
        screen: Arc<RwLock<Box<Screen>>>,
        time: Instant,
    },

    /// Sent by ui thread. contains user input information.
    InputEvents {
        events: Vec<InputEvent>,
    },

    /// Sent core -> worker thread.
    /// Saving the document's data is done in parallel in a thread.
    /// The use can still browse the document.
    SyncTask {
        doc: Arc<RwLock<Document<'a>>>,
    },
    // test
    IndexTask {
        document_map: Arc<RwLock<HashMap<document::Id, Arc<RwLock<Document<'a>>>>>>,
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
    pub button: u32, // TODO:: use enum MouseButton { Left, Middle , Right } ?
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
    RefreshUi { width: usize, height: usize }, // resize
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
/// for ButtonEvent we do partial hashing , ie: we ignore the coordinates of the pointer
/// for other events we rely on #[derive(Hash)]
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

        InputEvent::WheelUp { mods, x: _, y: _ } => {
            "WheelUp".hash(&mut s);
            // ignore x y
            // (*x).hash(&mut s);
            // (*y).hash(&mut s);
            (*mods).hash(&mut s)
        }

        InputEvent::WheelDown { mods, x: _, y: _ } => {
            "WheelDown".hash(&mut s);
            // ignore x y
            // (*x).hash(&mut s);
            // (*y).hash(&mut s);
            (*mods).hash(&mut s)
        }

        InputEvent::PointerMotion(PointerEvent { mods, x: _, y: _ }) => {
            "PointerMotion".hash(&mut s);
            // ignore x y
            // (*x).hash(&mut s);
            // (*y).hash(&mut s);
            (*mods).hash(&mut s)
        }

        _ => t.hash(&mut s),
    }

    s.finish()
}
