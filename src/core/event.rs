use core::document;
use core::screen::Screen;
use core::view;

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
        ev: self::InputEvent,
    },
    /// Sent by ui thread. Request the rendering of a given view.
    RequestLayoutEvent {
        view_id: view::Id,
        doc_id: document::Id,
        screen: Box<Screen>,
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

/// Supported input events
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum InputEvent {
    InvalidInputEvent,
    NoInputEvent,
    KeyPress {
        key: Key,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    ButtonPress {
        button: u32,
        x: i32,
        y: i32,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    ButtonRelease {
        button: u32,
        x: i32,
        y: i32,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    PointerMotion {
        x: i32,
        y: i32,
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    WheelUp {
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
    WheelDown {
        ctrl: bool,
        alt: bool,
        shift: bool,
    },
}

/// List of supported keyboard keys
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum Key {
    NUL,
    UNICODE(char), // unicode val
    Tab,           /* '\t' move to unicode ? */
    Linefeed,
    Clear,
    Return, // '\n' ?
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
    F1,
    F2,
    F3,
    F4,
    F5,
    F6,
    F7,
    F8,
    F9,
    F10,
    F11,
    F12,
    ///
    KeypadPlus,
    KeypadMinus,
    KeypadMul,
    KeypadDiv,
    KeypadEnter,
    NoKey,
}
