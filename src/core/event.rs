use core::screen::Screen;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Event {
    InputEvent,
    RequestLayoutEvent {
        view: u32,
        screen: Box<Screen>,
    },
    BuildLayoutEvent {
        view: u32,
        screen: Box<Screen>,
    },
    SystemEvent,
    ApplicationEvent,
    ResizeEvent {
        view: u32,
        width: usize,
        height: usize,
    }, // CloseEvent{ view: u32 } ??
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
