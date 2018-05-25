use core::document;
use core::screen::Screen;
use core::view;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    RequestDocumentList,
    DocumentList {
        list: Vec<(document::Id, String)>,
    },

    CreateView {
        width: usize,
        height: usize,
        doc_id: document::Id,
    },
    ViewCreated {
        width: usize,
        height: usize,
        doc_id: document::Id,
        view_id: view::Id,
    },

    DestroyView {
        width: usize,
        height: usize,
        doc_id: document::Id,
        view_id: view::Id,
    },
    ViewDestroyed {
        width: usize,
        height: usize,
        doc_id: document::Id,
        view_id: view::Id,
    },

    InputEvent {
        ev: self::InputEvent,
    },

    RequestLayoutEvent {
        view_id: view::Id,
        doc_id: document::Id,
        screen: Box<Screen>,
    },
    BuildLayoutEvent {
        view_id: view::Id,
        doc_id: document::Id,
        screen: Box<Screen>,
    },
    SystemEvent,
    ApplicationEvent,
    ResizeEvent {
        view_id: view::Id,
        width: usize,
        height: usize,
    },
    ApplicationQuitEvent,
}

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
