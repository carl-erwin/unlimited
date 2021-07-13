use crate::dbg_println;

use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event,
    event::{DisableMouseCapture, EnableMouseCapture},
    queue,
    style::Styler,
    style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    Result,
};

use crossterm::{
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};

use std::vec::Vec;

use std::thread;

use std::time::Duration;
use std::time::Instant;

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

//
//
use crate::core::event::ButtonEvent;
use crate::core::event::Event;
use crate::core::event::Event::*;
use crate::core::event::PointerEvent;

use crate::core::event::EventMessage;
use crate::core::event::InputEvent;
use crate::core::screen::Screen;

//use crate::core::event::InputEvent::*;
use crate::core::event::Key;
//use crate::core::event::Key::*;

use crate::core::event::KeyModifiers;

use crate::core::codepointinfo::CodepointInfo;

//
use crate::ui::UiState;

fn stdin_thread(core_tx: &Sender<EventMessage>, ui_tx: &Sender<EventMessage>) {
    // TODO: generate_test from logs grep | awk >>
    //    let v = autotest_0001();
    //    send_input_events(&v, &tx);

    loop {
        get_input_events(&core_tx, &ui_tx).unwrap();
    }
}

// fn autotest_0001() -> Vec<InputEvent> {
// }

pub fn main_loop(
    ui_rx: &Receiver<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) -> Result<()> {
    let mut seq: usize = 0;

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;
        *seq
    }

    let core_tx_clone = core_tx.clone();
    let ui_tx_clone = ui_tx.clone();

    thread::spawn(move || {
        stdin_thread(&core_tx_clone, &ui_tx_clone);
        return;
    });

    // ui state
    let mut ui_state = UiState::new();

    // ui ctx : TODO move to struct UiCtx
    let mut last_screen = Box::new(Screen::new(0, 0));
    let mut last_screen_rdr_time = Instant::now();

    let stdout = stdout();
    let mut stdout = stdout.lock();

    execute!(stdout, EnterAlternateScreen)?;

    execute!(
        stdout,
        EnableMouseCapture, // TODO: add option for mouse capture --(en|dis)able-mouse
        Hide,
        SetAttribute(Attribute::Reset),
        Clear(ClearType::All)
    )?;

    crossterm::terminal::enable_raw_mode()?;

    // first request
    // check terminal size
    let (width, height) = crossterm::terminal::size().ok().unwrap();
    let msg = EventMessage::new(
        get_next_seq(&mut seq),
        Event::UpdateViewEvent {
            width: width as usize,
            height: height as usize,
        },
    );
    core_tx.send(msg).unwrap_or(()); // if removed not screen is displayed

    while !ui_state.quit {
        if let Ok(evt) = ui_rx.recv() {
            match evt.event {
                Event::ApplicationQuitEvent => {
                    ui_state.quit = true;
                    let msg =
                        EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
                    core_tx.send(msg).unwrap_or(());
                    break;
                }

                UpdateViewEvent { width, height } => {
                    let msg = EventMessage::new(
                        get_next_seq(&mut seq),
                        Event::UpdateViewEvent { width, height },
                    );
                    core_tx.send(msg).unwrap_or(());
                }

                DrawEvent { screen, time: _ } => {
                    let start = Instant::now();
                    let mut draw = false;

                    let p_input = crate::core::event::pending_input_event_count();
                    let p_rdr = crate::core::event::pending_render_event_count();

                    //                    dbg_println!("DRAW: crossterm pre rdr : p_input {}\r", p_input);
                    //                    dbg_println!("DRAW: crossterm pre rdr : p_rdr {}\r", p_rdr);

                    if p_input < 25 && p_rdr < 25 {
                        draw = true;
                        dbg_println!("DRAW: crossterm DRAW frame ----- \r");
                    }

                    let diff = (start - last_screen_rdr_time).as_millis();
                    //                    dbg_println!("DRAW: crossterm diff {} ----- \r", diff);
                    if diff >= 1000 / 10 {
                        draw = true;
                        //                        dbg_println!("DRAW: crossterm DRAW timeout frame ----- \r");
                    }

                    //draw = true;

                    if draw {
                        let screen = screen.read().unwrap();
                        let mut screen = screen.clone();

                        draw_view(&mut last_screen, &mut screen, &mut stdout);

                        last_screen = screen;

                        last_screen_rdr_time = Instant::now();
                    } else {
                        dbg_println!("DRAW: crossterm SKIP frame ----- \r");
                    }

                    let end = Instant::now();
                    let p_rdr = crate::core::event::pending_render_event_dec(1);

                    dbg_println!(
                        "DRAW: crossterm : time spent to draw view = {} µs | p_rdr {}\r",
                        (end - start).as_micros(),
                        p_rdr
                    );
                }

                _ => {}
            }
        }
    }

    /* Terminate crossterm */
    execute!(
        stdout,
        SetAttribute(Attribute::Reset),
        Clear(ClearType::All),
        DisableMouseCapture,
        Show
    )?;

    execute!(stdout, LeaveAlternateScreen,)?;

    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idiomatic    func()? style
*/
fn draw_view(
    mut last_screen: &mut Screen,
    mut screen: &mut Screen,
    mut stdout: &mut std::io::StdoutLock,
) {
    if !true {
        let _ = draw_screen(&mut last_screen, &mut screen, &mut stdout);
    } else {
        let _ = _draw_screen_dumb(&screen, &mut stdout);
    }
}

fn _draw_screen_dumb(screen: &Screen, stdout: &mut std::io::StdoutLock) -> Result<()> {
    // queue!(stdout, ResetColor)?;
    //queue!(stdout, Clear(ClearType::All))?;

    for li in 0..screen.height() {
        queue!(stdout, MoveTo(0, li as u16))?;
        //queue!(stdout, Clear(ClearType::CurrentLine))?;

        let line = screen.get_line(li).unwrap();

        for c in 0..line.width() {
            let cpi = line.get_cpi(c).unwrap();

            // color
            let color = Color::Rgb {
                r: cpi.style.color.0,
                g: cpi.style.color.1,
                b: cpi.style.color.2,
            };
            // color
            let bg_color = Color::Rgb {
                r: cpi.style.bg_color.0,
                g: cpi.style.bg_color.1,
                b: cpi.style.bg_color.2,
            };

            // draw with style
            let s = cpi.displayed_cp.to_string();
            if cpi.style.is_inverse {
                queue!(
                    stdout,
                    SetBackgroundColor(bg_color),
                    SetForegroundColor(color),
                    ::crossterm::style::PrintStyledContent(s.reverse())
                )?;
            } else {
                queue!(
                    stdout,
                    SetBackgroundColor(bg_color),
                    SetForegroundColor(color),
                    Print(cpi.displayed_cp)
                )?;
            }
        }
    }

    /* Update the screen. */
    stdout.flush()?;

    Ok(())
}

fn screen_changed(screen0: &Screen, screen1: &Screen) -> bool {
    let nbp = screen0.push_count != screen1.push_count;
    let o = screen0.first_offset != screen1.first_offset;
    let w = screen0.max_width() != screen1.max_width();
    let h = screen0.max_height() != screen1.max_height();
    nbp || o || w || h
}

fn screen_width_change(screen0: &Screen, screen1: &Screen) -> bool {
    screen0.max_width() != screen1.max_width()
}

fn screen_height_change(screen0: &Screen, screen1: &Screen) -> bool {
    screen0.max_height() != screen1.max_height()
}

fn cpis_have_same_style(a: &CodepointInfo, b: &CodepointInfo) -> bool {
    // pub cp: char,
    let dcp = a.displayed_cp == b.displayed_cp;
    let same_style = a.style == b.style;

    dcp && same_style
}

fn draw_screen(
    last_screen: &mut Screen,
    screen: &mut Screen,
    stdout: &mut std::io::StdoutLock,
) -> Result<()> {
    let mut prev_cpi = CodepointInfo::new();

    let screen_change = screen_changed(&last_screen, &screen);
    let width_change = screen_width_change(&last_screen, &screen);
    let _height_change = screen_height_change(&last_screen, &screen);

    let check_hash = !screen_change;
    let column_change = width_change;

    // set default color
    if false {
        let color = Color::Rgb {
            r: prev_cpi.style.color.0,
            g: prev_cpi.style.color.1,
            b: prev_cpi.style.color.2,
        };
        let bg_color = Color::Rgb {
            r: prev_cpi.style.bg_color.0,
            g: prev_cpi.style.bg_color.1,
            b: prev_cpi.style.bg_color.2,
        };

        queue!(
            stdout,
            SetAttribute(Attribute::Reset),
            SetForegroundColor(color),
            SetBackgroundColor(bg_color)
        )?;
    }

    // dbg_println!("check_hash = {}", check_hash);

    // current style
    for l in 0..screen.max_height() {
        queue!(stdout, MoveTo(0, l as u16))?;

        let line = screen.get_unclipped_line_mut(l).unwrap();

        if check_hash {
            let prev_line = last_screen.get_unclipped_line_mut(l).unwrap();
            if prev_line.hash() == line.hash() {
                //dbg_println!("line[{}] SKIP ...", l);
                continue;
            }
        }

        /////////////////////
        // draw line
        /////////////////////

        let mut set_style = true;
        let mut set_color = true;

        let mut nb_draw_char = 0;
        let mut nb_skip_char = 0;

        for c in 0..line.max_width() {
            let cpi = line.get_unclipped_cpi(c).unwrap();

            if cpi.skip_render {
                queue!(stdout, MoveTo(c as u16 + 1, l as u16))?;
                continue;
            }

            let mut change = c == 0;

            if change {
                set_style = true;
                set_color = true;
            }

            // default style
            if cpi.style.is_inverse != prev_cpi.style.is_inverse {
                set_style = true;
                set_color = true;
                change = true;
            }

            // detect color change
            if prev_cpi.style.color != cpi.style.color
                || prev_cpi.style.bg_color != cpi.style.bg_color
            {
                set_color = true;
                change = true;
            }

            prev_cpi = *cpi;

            if !column_change && !change {
                if let Some(prev_line) = last_screen.get_unclipped_line_mut(l) {
                    if let Some(prev_screen_cpi) = prev_line.get_unclipped_cpi(c) {
                        if cpis_have_same_style(cpi, prev_screen_cpi) {
                            queue!(stdout, MoveTo((c + 1) as u16, l as u16))?;
                            nb_skip_char += 1;
                            continue;
                        }
                    }
                }
            }

            nb_draw_char += 1;

            // draw
            {
                if set_style {
                    set_style = false;
                    if cpi.style.is_inverse {
                        queue!(stdout, SetAttribute(Attribute::Reverse))?;
                    } else {
                        queue!(stdout, SetAttribute(Attribute::NoReverse))?;
                    }
                }

                if set_color {
                    set_color = false;
                    let color = Color::Rgb {
                        r: cpi.style.color.0,
                        g: cpi.style.color.1,
                        b: cpi.style.color.2,
                    };

                    let bg_color = Color::Rgb {
                        r: cpi.style.bg_color.0,
                        g: cpi.style.bg_color.1,
                        b: cpi.style.bg_color.2,
                    };
                    queue!(
                        stdout,
                        SetForegroundColor(color),
                        SetBackgroundColor(bg_color)
                    )?;
                }

                // draw character
                queue!(stdout, Print(cpi.displayed_cp))?;
            }
        }

        if false {
            // env.screen.debug ?
            dbg_println!(
                "line[{}] DRAW : real({}) skip({}) *** ",
                l,
                nb_draw_char,
                nb_skip_char
            );
        }
    }

    // Update the screen
    stdout.flush()?;

    Ok(())
}

fn translate_crossterm_key_modifier(km: ::crossterm::event::KeyModifiers) -> KeyModifiers {
    KeyModifiers {
        ctrl: (km.bits() & event::KeyModifiers::CONTROL.bits()) != 0,
        alt: (km.bits() & event::KeyModifiers::ALT.bits()) != 0,
        shift: (km.bits() & event::KeyModifiers::SHIFT.bits()) != 0,
    }
}

fn key_modifiers_no_shift(km: ::crossterm::event::KeyModifiers) -> KeyModifiers {
    KeyModifiers {
        ctrl: (km.bits() & event::KeyModifiers::CONTROL.bits()) != 0,
        alt: (km.bits() & event::KeyModifiers::ALT.bits()) != 0,
        shift: false,
    }
}

fn translate_crossterm_mouse_button(button: ::crossterm::event::MouseButton) -> u32 {
    match button {
        ::crossterm::event::MouseButton::Left => 0,
        ::crossterm::event::MouseButton::Right => 1,
        ::crossterm::event::MouseButton::Middle => 2,
        // self::termion::event::MouseButton::WheelUp => 3,
        // self::termion::event::MouseButton::WheelDown => 4,
    } //
}

fn translate_crossterm_event(
    evt: ::crossterm::event::Event,
    pending_resize: &mut bool,
) -> InputEvent {
    // translate termion event
    *pending_resize = false;
    //    dbg_println!("CROSSTERM EVENT : {:?}", evt);

    match evt {
        ::crossterm::event::Event::Key(ke) => match ke.code {
            ::crossterm::event::KeyCode::Char(c) => {
                return InputEvent::KeyPress {
                    mods: key_modifiers_no_shift(ke.modifiers),
                    key: Key::Unicode(c),
                };
            }

            ::crossterm::event::KeyCode::Backspace => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::BackSpace,
                };
            }

            ::crossterm::event::KeyCode::Enter => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Unicode('\n'),
                };
            }

            ::crossterm::event::KeyCode::Left => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Left,
                };
            }

            ::crossterm::event::KeyCode::Right => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Right,
                };
            }

            ::crossterm::event::KeyCode::Up => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Up,
                };
            }
            ::crossterm::event::KeyCode::Down => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Down,
                };
            }

            ::crossterm::event::KeyCode::Home => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Home,
                };
            }

            ::crossterm::event::KeyCode::End => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::End,
                };
            }

            ::crossterm::event::KeyCode::PageUp => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::PageUp,
                };
            }
            ::crossterm::event::KeyCode::PageDown => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::PageDown,
                };
            }

            ::crossterm::event::KeyCode::Tab => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Unicode('\t'),
                };
            }

            ::crossterm::event::KeyCode::BackTab => {
                return InputEvent::NoInputEvent;
            }

            ::crossterm::event::KeyCode::Delete => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Delete,
                };
            }

            ::crossterm::event::KeyCode::Insert => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Insert,
                };
            }

            ::crossterm::event::KeyCode::F(n) => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::F(n as usize),
                };
            }

            ::crossterm::event::KeyCode::Null => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Unicode('\0'),
                };
            }

            ::crossterm::event::KeyCode::Esc => {
                return InputEvent::KeyPress {
                    mods: translate_crossterm_key_modifier(ke.modifiers),
                    key: Key::Escape,
                };
            }
        },

        ::crossterm::event::Event::Mouse(event) => match event.kind {
            ::crossterm::event::MouseEventKind::Down(button) => {
                return InputEvent::ButtonPress(ButtonEvent {
                    mods: translate_crossterm_key_modifier(event.modifiers),
                    x: i32::from(event.column),
                    y: i32::from(event.row),
                    button: translate_crossterm_mouse_button(button),
                });
            }

            ::crossterm::event::MouseEventKind::Up(button) => {
                return InputEvent::ButtonRelease(ButtonEvent {
                    mods: translate_crossterm_key_modifier(event.modifiers),
                    x: i32::from(event.column),
                    y: i32::from(event.row),
                    button: translate_crossterm_mouse_button(button),
                });
            }

            ::crossterm::event::MouseEventKind::ScrollUp => {
                return InputEvent::WheelUp {
                    mods: translate_crossterm_key_modifier(event.modifiers),
                    x: i32::from(event.column),
                    y: i32::from(event.row),
                };
            }

            ::crossterm::event::MouseEventKind::ScrollDown => {
                return InputEvent::WheelDown {
                    mods: translate_crossterm_key_modifier(event.modifiers),
                    x: i32::from(event.column),
                    y: i32::from(event.row),
                };
            }

            ::crossterm::event::MouseEventKind::Drag(_button) => {
                // TODO: no Drag event in the editor yet ?
                // TODO: filter dragged button

                // return InputEvent::NoInputEvent;

                dbg_println!("DRAG");

                return InputEvent::PointerMotion(PointerEvent {
                    mods: translate_crossterm_key_modifier(event.modifiers),
                    x: i32::from(event.column),
                    y: i32::from(event.row),
                });
            }

            ::crossterm::event::MouseEventKind::Moved => {
                // return InputEvent::NoInputEvent;

                return InputEvent::PointerMotion(PointerEvent {
                    mods: translate_crossterm_key_modifier(event.modifiers),
                    x: i32::from(event.column),
                    y: i32::from(event.row),
                });
            }
        },

        ::crossterm::event::Event::Resize(width, height) => {
            // println!("New size {}x{}", width, height)
            // TODO: not really an input
            *pending_resize = true;
            return InputEvent::RefreshUi {
                width: width as usize,
                height: height as usize,
            };
        }
    }

    // return InputEvent::NoInputEvent;
}

fn send_input_events(
    accum: &Vec<InputEvent>,
    tx: &Sender<EventMessage>,
    _ui_tx: &Sender<EventMessage>,
) {
    let mut v = Vec::<InputEvent>::new();

    // merge consecutive characters as "array" of chars
    let mut codepoints = Vec::<char>::new();

    if accum.len() == 1 {
        match accum[0] {
            InputEvent::RefreshUi { width, height } => {
                let msg = EventMessage::new(0, Event::UpdateViewEvent { width, height });

                // ui_tx.send(msg).unwrap_or(()); ?

                // send to core
                crate::core::event::pending_input_event_inc(1);
                tx.send(msg).unwrap_or(());
            }

            _ => {
                // send
                let msg = EventMessage::new(
                    0,
                    Event::InputEvents {
                        events: accum.clone(),
                    },
                );
                crate::core::event::pending_input_event_inc(1);
                tx.send(msg).unwrap_or(());
                return;
            }
        }
        return;
    }

    let mut refresh = false;
    let mut new_width = 0;
    let mut new_height = 0;

    for evt in accum {
        match evt {
            InputEvent::RefreshUi { width, height } => {
                refresh = true;
                new_width = *width;
                new_height = *height;
            }

            InputEvent::KeyPress {
                key: Key::Unicode(c),
                mods:
                    KeyModifiers {
                        ctrl: false,
                        alt: false,
                        shift: false,
                    },
            } => {
                codepoints.push(*c);
            }

            _ => {
                // flush previous codepoints
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

                // other events
                v.push(evt.clone());
            }
        }
    }

    // resize are urgent
    if refresh {
        let msg = EventMessage::new(
            0,
            Event::UpdateViewEvent {
                width: new_width,
                height: new_height,
            },
        );
        tx.send(msg).unwrap_or(());
    }

    // append
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

    // send
    if !v.is_empty() {
        let ev_count = v.len();
        let msg = EventMessage::new(0, Event::InputEvents { events: v });
        crate::core::event::pending_input_event_inc(ev_count);
        tx.send(msg).unwrap_or(());
    }
}

/*
  NB: There is a subtle bug in crossterm input handling.

      - Level-triggered polling was removed from mio (in 0.7.xx version)
      - On linux the (default) 0 1 2 fd points to the same pseudo terminal
        And thus we cannot change the blocking mode of the input fd (0)

      - When pasting big chunks of text with graphical terminal. The editor seams "frozen".
        because the input file descriptor is in blocking mode.

        if the user input it exactly the size of crossterm's internal buffer, the next call to 'read' will block.
        Because the internal buffer is full, crossterm expect more bytes and loops on "read"

      - It is not possible to use println!() function family in non-blocking mode.
       println!() must ensure the data is flushed and will panic on EAGAIN error.

       *) One solution is for crossterm to let the user specify the input buffer/size (compile time ?)
         In the case of unlimitED we could use a 2M input buffer ?

       *) An other solution (hack) (my fork on github)
        change input fd from blocking to no-blocking mode, do read loop and restore mode on exit.
*/
fn get_input_events(
    tx: &Sender<EventMessage>,
    ui_tx: &Sender<EventMessage>,
) -> ::crossterm::Result<()> {
    let mut accum = Vec::<InputEvent>::with_capacity(255);
    let mut wait_ms = 60_000;
    let mut min_wait_ms = 2;

    let mut start = Instant::now();
    let mut prev_ev_time = start;

    let mut count = 0;
    let mut pending_resize = false;
    loop {
        if ::crossterm::event::poll(Duration::from_millis(wait_ms))? {
            if let Ok(cross_evt) = ::crossterm::event::read() {
                prev_ev_time = Instant::now();
                let evt = translate_crossterm_event(cross_evt, &mut pending_resize);
                accum.push(evt);
                if pending_resize {
                    min_wait_ms = 16; // wait for over resize events
                }
            }
        }

        count += 1;
        wait_ms = min_wait_ms;
        if count == 1 {
            // delay flush of 1st input event (min_wait_ms)
            // real start
            start = Instant::now();
            continue;
        }

        let d = prev_ev_time.elapsed();
        if d < Duration::from_millis(1) || start.elapsed() < Duration::from_millis(min_wait_ms) {
            // batch input
            continue;
        }

        break;
    }

    if !accum.is_empty() {
        send_input_events(&accum, tx, ui_tx);
    }

    Ok(())
}
