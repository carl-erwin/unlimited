// module export
pub mod input_map;

use parking_lot::RwLock;
use std::cell::RefCell;
use std::collections::hash_map::DefaultHasher;
use std::collections::HashMap;
use std::hash::{Hash, Hasher};
use std::rc::Rc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::vec::Vec;

use crate::core::buffer;
use crate::core::buffer::Buffer;
use crate::core::buffer::BufferEvent;

use crate::core::screen::Screen;

//
// TODO(ceg): implement functions to update the counters
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
pub struct Message<'a> {
    /// sequence number. should be reused in corresponding answer.
    pub seq: usize,
    // timestamp since application startup (in milliseconds) of the source/input Message which triggered this response message or 0
    pub input_ts: u128,
    // timestamp since application startup (in milliseconds) of this Message or 0
    pub ts: u128,
    /// underlying event.
    pub event: Event<'a>,
    // pub reply_to: Sender<Message>, // clone
}

impl<'a> Message<'a> {
    pub fn new(seq: usize, input_ts: u128, ts: u128, event: Event<'a>) -> Self {
        Message {
            seq,
            input_ts,
            ts,
            event,
        }
    }
}

/// Events sent between core and ui threads via Message encapsulation.
#[derive(Debug, Clone)]
pub enum Event<'a> {
    /// Sent by ui thread. Request the rendering of a given view.
    UpdateView {
        width: usize,  // used to detect change
        height: usize, // used to detect change
    },

    /// Sent to core thread to update the ui
    RefreshView,

    /// Sent by core thread. Contains the rendered screen that maps view_id.
    Draw {
        screen: Arc<RwLock<Box<Screen>>>,
    },

    /// Sent by ui thread. contains user input information.
    Input {
        events: Vec<InputEvent>,
    },

    /// Sent core -> worker thread.
    /// Saving the buffer's data is done in parallel in a thread.
    /// The user can still browse the buffer.
    SyncTask {
        buffer: Arc<RwLock<Buffer<'a>>>,
    },
    // test
    IndexTask {
        buffer_map: Arc<RwLock<HashMap<buffer::Id, Arc<RwLock<Buffer<'a>>>>>>,
    },

    Buffer {
        event: BufferEvent,
    },

    ApplicationQuit,
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
    pub button: u32, // TODO(ceg):: use enum MouseButton { Left, Middle , Right } ?
    pub x: i32,
    pub y: i32,
    pub mods: KeyModifiers,
}

impl Hash for ButtonEvent {
    // special hash for ButtonPress/ButtonRelease that ignores (x,y)
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

/// Supported input events
#[derive(Hash, Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    DummyInputEvent,
    FallbackEvent, // use to map default action in input table
    UiResized { width: usize, height: usize }, // resize
    KeyPress { key: Key, mods: KeyModifiers },
    KeyRelease { key: Key, mods: KeyModifiers },
    ButtonPress(ButtonEvent),
    ButtonRelease(ButtonEvent),
    PointerMotion(PointerEvent),
    WheelUp { mods: KeyModifiers, x: i32, y: i32 },
    WheelDown { mods: KeyModifiers, x: i32, y: i32 },
    Paste(String),
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
                button.hash(&mut s);
                // ignore x y
                // x.hash(&mut s);
                // y.hash(&mut s);
                mods.hash(&mut s)
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
                button.hash(&mut s);
                // ignore x y
                // x.hash(&mut s);
                // y.hash(&mut s);
                mods.hash(&mut s)
            }
        },

        InputEvent::WheelUp { mods, x: _, y: _ } => {
            "WheelUp".hash(&mut s);
            // ignore x y
            // x.hash(&mut s);
            // y.hash(&mut s);
            mods.hash(&mut s)
        }

        InputEvent::WheelDown { mods, x: _, y: _ } => {
            "WheelDown".hash(&mut s);
            // ignore x y
            // x.hash(&mut s);
            // y.hash(&mut s);
            mods.hash(&mut s)
        }

        InputEvent::PointerMotion(PointerEvent { mods, x: _, y: _ }) => {
            "PointerMotion".hash(&mut s);
            // ignore x y
            // x.hash(&mut s);
            // y.hash(&mut s);
            mods.hash(&mut s)
        }

        _ => t.hash(&mut s),
    }

    s.finish()
}
