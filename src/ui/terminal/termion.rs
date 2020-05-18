//
use std::io::Error;
use std::io::{self, Stdout, Write};

use std::thread;
use std::time::Duration;
use std::time::Instant;

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;
use std::sync::RwLock;

extern crate libc;

use self::libc::{c_void, read};

//
extern crate termion;

use crate::dbg_println;

use self::termion::event::parse_event;
use self::termion::input::MouseTerminal;
use self::termion::raw::IntoRawMode;
use self::termion::screen::{AlternateScreen, ToMainScreen};
use self::termion::terminal_size;

//
use crate::core::event::Event;
use crate::core::event::Event::*;
use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::event::{ButtonEvent, PointerEvent};

use crate::core::screen::Screen;

use crate::core::event::Key;
use crate::core::event::KeyModifiers;

//
use crate::ui::UiState;

fn stdin_thread(tx: &Sender<EventMessage>) {
    loop {
        get_input_events(&tx);
    }
}

pub fn main_loop<'a>(
    ui_rx: &Receiver<EventMessage<'static>>,
    _ui_tx: &Sender<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;
        *seq
    }

    // front-end init code {----
    // init termion
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);

    let core_tx_clone = core_tx.clone();
    thread::spawn(move || {
        stdin_thread(&core_tx_clone);
    });

    // ui state
    let mut ui_state = UiState::new();

    // ui ctx : TODO move to struct UiCtx
    let mut last_screen = Arc::new(RwLock::new(Box::new(Screen::new(1, 1)))); // last screen ?
    let mut last_screen_rdr_time = Instant::now();
    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();

    let mut request_layout = true;

    while !ui_state.quit {
        // check terminal size
        let (width, height) = terminal_size().unwrap();

        if ui_state.terminal_width != width || ui_state.terminal_height != height {
            ui_state.terminal_width = width;
            ui_state.terminal_height = height;
            request_layout = true;
        }

        // need layout ?
        if request_layout {
            let msg = EventMessage::new(
                get_next_seq(&mut seq),
                Event::UpdateViewEvent {
                    width: ui_state.terminal_width as usize,
                    height: ui_state.terminal_height as usize,
                },
            );
            core_tx.send(msg).unwrap_or(());
            request_layout = false;
        }

        if let Ok(evt) = ui_rx.recv_timeout(Duration::from_millis(1000)) {
            match evt.event {
                Event::ApplicationQuitEvent => {
                    ui_state.quit = true;
                    let msg =
                        EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
                    core_tx.send(msg).unwrap_or(());
                    break;
                }

                DrawEvent { screen, time: _ } => {
                    let start = Instant::now();
                    let mut draw = false;

                    crate::core::event::pending_render_event_dec(1);

                    let p_input = crate::core::event::pending_input_event_count();
                    let p_rdr = crate::core::event::pending_render_event_count();

                    dbg_println!("DRAW: crossterm pre rdr : p_input {}\r", p_input);
                    dbg_println!("DRAW: crossterm pre rdr : p_rdr {}\r", p_rdr);

                    if p_input < 10 && p_rdr < 10 {
                        draw = true;
                        dbg_println!("DRAW: crossterm DRAW frame ----- \r");
                    }

                    let diff = (start - last_screen_rdr_time).as_millis();
                    dbg_println!("DRAW: crossterm diff {} ----- \r", diff);
                    if diff >= 1000 / 60 {
                        draw = true;
                        dbg_println!("DRAW: crossterm DRAW timeout frame ----- \r");
                    }

                    if draw {
                        {
                            let mut screen = screen.write().unwrap();
                            let mut last_screen = last_screen.write().unwrap();
                            draw_view(&mut last_screen, &mut screen, &mut stdout);
                        }
                        last_screen = screen;
                        last_screen_rdr_time = Instant::now();
                    } else {
                        dbg_println!("DRAW: crossterm SKIP frame ----- \r");
                    }

                    let end = Instant::now();
                    dbg_println!(
                        "DRAW: crossterm : time to draw view = {} µs\r",
                        (end - start).as_micros()
                    );
                    let p_rdr = crate::core::event::pending_render_event_count();
                    dbg_println!("DRAW: crossterm post rdr : p_rdr {}\r", p_rdr);
                }

                _ => {}
            }
        } else {
            // TODO: handle timeout
        }
    }

    // front-end quit code {----
    // on quit, clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    write!(stdout, "{}{}", ToMainScreen, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
    // ----}
}

fn draw_screen(_last_screen: &mut Screen, screen: &mut Screen, mut stdout: &mut Stdout) {
    write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();
    write!(stdout, "{}", termion::style::Reset).unwrap();

    for l in 0..screen.height() {
        let line = screen.get_line_mut(l).unwrap();

        terminal_cursor_to(&mut stdout, 1, (1 + l) as u16);

        for c in line {
            let cpi = c.cpi;

            if cpi.style.is_inverse {
                write!(stdout, "{}", termion::style::Invert).unwrap();
            } else {
                write!(stdout, "{}", termion::style::NoInvert).unwrap();
            }

            let fg = &cpi.style.color;
            let bg = &cpi.style.bg_color;

            write!(
                stdout,
                "{}{}{}",
                termion::color::Fg(termion::color::Rgb(fg.0, fg.1, fg.2)),
                termion::color::Bg(termion::color::Rgb(bg.0, bg.1, bg.2)),
                cpi.displayed_cp
            )
            .unwrap();
        }
    }
    stdout.flush().unwrap();
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(last_screen: &mut Screen, mut screen: &mut Screen, mut stdout: &mut Stdout) {
    draw_screen(last_screen, &mut screen, &mut stdout);
}

fn _terminal_clear_current_line(stdout: &mut Stdout, line_width: u16) {
    for _ in 0..line_width {
        write!(stdout, " ").unwrap();
    }
}

fn terminal_cursor_to(stdout: &mut Stdout, x: u16, y: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y)).unwrap();
}

fn translate_termion_event(evt: self::termion::event::Event) -> InputEvent {
    // translate termion event
    match evt {
        self::termion::event::Event::Key(k) => match k {
            self::termion::event::Key::Ctrl(c) => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: true,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Unicode(c),
                };
            }

            self::termion::event::Key::Char(c) => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Unicode(c),
                };
            }

            self::termion::event::Key::Alt(c) => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: true,
                        shift: false,
                    },
                    key: Key::Unicode(c),
                };
            }

            self::termion::event::Key::F(n) => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::F(n as usize),
                };
            }

            self::termion::event::Key::Left => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Left,
                };
            }
            self::termion::event::Key::Right => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Right,
                };
            }
            self::termion::event::Key::Up => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Up,
                };
            }
            self::termion::event::Key::Down => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Down,
                };
            }
            self::termion::event::Key::Backspace => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::BackSpace,
                };
            }
            self::termion::event::Key::Home => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Home,
                };
            }
            self::termion::event::Key::End => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::End,
                };
            }
            self::termion::event::Key::PageUp => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::PageUp,
                };
            }
            self::termion::event::Key::PageDown => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::PageDown,
                };
            }
            self::termion::event::Key::Delete => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Delete,
                };
            }
            self::termion::event::Key::Insert => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Insert,
                };
            }
            self::termion::event::Key::Esc => {
                return InputEvent::KeyPress {
                    mods: KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
                    key: Key::Escape,
                };
            }
            _ => {}
        },

        self::termion::event::Event::Mouse(m) => {
            fn termion_mouse_button_to_u32(mb: self::termion::event::MouseButton) -> u32 {
                match mb {
                    self::termion::event::MouseButton::Left => 0,
                    self::termion::event::MouseButton::Right => 1,
                    self::termion::event::MouseButton::Middle => 2,
                    self::termion::event::MouseButton::WheelUp => 3,
                    self::termion::event::MouseButton::WheelDown => 4,
                }
            }

            match m {
                self::termion::event::MouseEvent::Press(mb, x, y) => {
                    let button = termion_mouse_button_to_u32(mb);

                    if button == 3 {
                        return InputEvent::WheelUp {
                            mods: KeyModifiers {
                                ctrl: false,
                                alt: false,
                                shift: false,
                            },
                            x: i32::from(x - 1),
                            y: i32::from(y - 1),
                        };
                    }

                    if button == 4 {
                        return InputEvent::WheelDown {
                            mods: KeyModifiers {
                                ctrl: false,
                                alt: false,
                                shift: false,
                            },
                            x: i32::from(x - 1),
                            y: i32::from(y - 1),
                        };
                    }

                    return InputEvent::ButtonPress(ButtonEvent {
                        mods: KeyModifiers {
                            ctrl: false,
                            alt: false,
                            shift: false,
                        },
                        x: i32::from(x - 1),
                        y: i32::from(y - 1),
                        button,
                    });
                }

                self::termion::event::MouseEvent::Release(x, y) => {
                    return InputEvent::ButtonRelease(ButtonEvent {
                        mods: KeyModifiers {
                            ctrl: false,
                            alt: false,
                            shift: false,
                        },
                        x: i32::from(x - 1),
                        y: i32::from(y - 1),
                        button: 0xff,
                    });
                }

                self::termion::event::MouseEvent::Hold(x, y) => {
                    return InputEvent::PointerMotion(PointerEvent {
                        mods: KeyModifiers {
                            ctrl: false,
                            alt: false,
                            shift: false,
                        },
                        x: i32::from(x - 1),
                        y: i32::from(y - 1),
                    });
                }
            };
        }

        self::termion::event::Event::Unsupported(_e) => {}
    }

    crate::core::event::InputEvent::NoInputEvent
}

fn get_input_events(tx: &Sender<EventMessage>) {
    const BUF_SIZE: usize = 1024 * 32;

    let mut buf = Vec::<u8>::with_capacity(BUF_SIZE);
    unsafe {
        buf.set_len(BUF_SIZE);
    }

    loop {
        let nb_read = unsafe { read(0, buf.as_mut_ptr() as *mut c_void, BUF_SIZE) as usize };
        let mut buf2 = Vec::<Result<u8, Error>>::with_capacity(nb_read);

        for b in buf.iter().take(nb_read) {
            buf2.push(Ok(*b));
        }

        let mut raw_evt = Vec::<_>::with_capacity(BUF_SIZE);
        let mut it = buf2.into_iter();

        while let Some(val) = it.next() {
            if let Ok(evt) = parse_event(val.unwrap(), &mut it) {
                raw_evt.push(evt);
            } else {
                break;
            }
        }

        // merge consecutive events
        let mut v = vec![];
        let mut codepoints = Vec::<char>::new();

        for evt in &raw_evt {
            let evt = translate_termion_event(evt.clone());
            match evt {
                InputEvent::KeyPress {
                    key: Key::Unicode(c),
                    mods:
                        KeyModifiers {
                            ctrl: false,
                            alt: false,
                            shift: false,
                        },
                } => {
                    codepoints.push(c);
                }

                _ => {
                    if !codepoints.is_empty() {
                        v.push(InputEvent::KeyPress {
                            key: Key::UnicodeArray(codepoints),
                            mods: KeyModifiers {
                                ctrl: false,
                                alt: false,
                                shift: false,
                            },
                        });
                        codepoints = Vec::<char>::new();
                    }
                    v.push(evt);
                }
            }
        }

        // send
        if !codepoints.is_empty() {
            v.push(InputEvent::KeyPress {
                key: Key::UnicodeArray(codepoints),
                mods: KeyModifiers {
                    ctrl: false,
                    alt: false,
                    shift: false,
                },
            });
        }

        if !v.is_empty() {
            let msg = EventMessage::new(0, Event::InputEvents { events: v });
            tx.send(msg).unwrap_or(());
        }
    }
}
