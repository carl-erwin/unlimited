use crate::dbg_println;

use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event,
    event::{DisableMouseCapture, EnableBracketedPaste, EnableFocusChange, EnableMouseCapture},
    execute, queue,
    style::{Attribute, Color, Print, SetAttribute, SetBackgroundColor, SetForegroundColor},
    terminal::{Clear, ClearType},
    terminal::{EnterAlternateScreen, LeaveAlternateScreen},
};

use crossterm::event::{
    KeyboardEnhancementFlags, PopKeyboardEnhancementFlags, PushKeyboardEnhancementFlags,
};

use std::vec::Vec;

use std::thread;

use std::time::Duration;
use std::time::Instant;

use parking_lot::RwLock;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;

//
//
use crate::core::config::ConfigVariables;

use crate::core::event::ButtonEvent;
use crate::core::event::Event;
use crate::core::event::Event::*;
use crate::core::event::PointerEvent;

use crate::core::event::InputEvent;
use crate::core::event::Message;
use crate::core::screen::Screen;

//use crate::core::event::InputEvent::*;
use crate::core::event::Key;
//use crate::core::event::Key::*;

use crate::core::event::KeyModifiers;

use crate::core::codepointinfo::CodepointInfo;

fn stdin_thread(core_tx: &Sender<Message>, ui_tx: &Sender<Message>) {
    // TODO(ceg): generate_test from logs grep | awk >>
    //    let v = autotest_0001();
    //    send_input_events(&v, &tx);

    let force_input = std::env::var("UNLIMITED_CROSSTERM_FORCE_INPUT").is_ok();

    loop {
        get_input_events(core_tx, ui_tx, force_input).unwrap();
    }
}

// fn autotest_0001() -> Vec<InputEvent> {
// }

pub fn main_loop(
    config_vars: &ConfigVariables,
    ui_rx: &Receiver<Message<'static>>,
    ui_tx: &Sender<Message<'static>>,
    core_tx: &Sender<Message<'static>>,
) -> Result<(), std::io::Error> {
    let mut draw_req = 0;
    let mut fps = 0;
    let mut drop = 0;
    let mut fps_t0 = Instant::now();

    let mut seq: usize = 0;

    let debug = config_vars
        .get(&"crossterm:debug".to_owned())
        .unwrap_or(&"".to_owned())
        == "1";

    fn get_next_seq(seq: &mut usize) -> usize {
        *seq += 1;
        *seq
    }

    let core_tx_clone = core_tx.clone();
    let ui_tx_clone = ui_tx.clone();

    thread::spawn(move || {
        stdin_thread(&core_tx_clone, &ui_tx_clone);
    });

    // ui ctx : TODO move to struct UiCtx
    let mut last_screen = Arc::new(RwLock::new(Box::new(Screen::new(0, 0))));

    let stdout = stdout();
    let mut stdout = stdout.lock();

    crossterm::terminal::enable_raw_mode()?;

    execute!(stdout, EnterAlternateScreen)?;

    let cpi = CodepointInfo::new();
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

    execute!(
        stdout,
        EnableMouseCapture, // TODO(ceg): add option for mouse capture --(en|dis)able-mouse
        EnableBracketedPaste,
        EnableFocusChange,
        Hide,
        SetAttribute(Attribute::Reset),
        SetBackgroundColor(bg_color),
        SetForegroundColor(color),
        Clear(ClearType::All)
    )?;

    //    let supports_keyboard_enhancement = matches!(
    //        crossterm::terminal::supports_keyboard_enhancement(),
    //        Ok(true)
    //    );
    let supports_keyboard_enhancement = true;

    if supports_keyboard_enhancement {
        execute!(
            stdout,
            PushKeyboardEnhancementFlags(
                KeyboardEnhancementFlags::DISAMBIGUATE_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALL_KEYS_AS_ESCAPE_CODES
                    | KeyboardEnhancementFlags::REPORT_ALTERNATE_KEYS
                    | KeyboardEnhancementFlags::REPORT_EVENT_TYPES
            )
        )?;
    }

    // first request
    // check terminal size
    let (width, height) = crossterm::terminal::size().ok().unwrap();

    let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();
    let msg = Message::new(
        get_next_seq(&mut seq),
        0,
        ts,
        Event::UpdateView {
            width: width as usize,
            height: height as usize,
        },
    );
    crate::core::event::pending_input_event_inc(1);
    core_tx.send(msg).unwrap_or(()); // if removed the 1st screen is not displayed

    let force_draw = std::env::var("UNLIMITED_CROSSTERM_FORCE_DRAW").is_ok();

    loop {
        if let Ok(msg) = ui_rx.recv() {
            let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

            match msg.event {
                Event::ApplicationQuit => {
                    let msg = Message::new(get_next_seq(&mut seq), 0, ts, Event::ApplicationQuit);
                    crate::core::event::pending_input_event_inc(1);
                    core_tx.send(msg).unwrap_or(());
                    break;
                }

                UpdateView { width, height } => {
                    let msg = Message::new(
                        get_next_seq(&mut seq),
                        0,
                        ts,
                        Event::UpdateView { width, height },
                    );
                    crate::core::event::pending_input_event_inc(1);
                    core_tx.send(msg).unwrap_or(());
                }

                Draw { screen } => {
                    draw_req += 1;

                    let start = Instant::now();

                    let p_rdr = crate::core::event::pending_render_event_count();
                    let p_input = crate::core::event::pending_input_event_count();

                    let mut draw = force_draw;

                    if crate::core::no_ui_render() {
                        draw = false;
                        fps += 1;
                    } else {
                        // force draw ?
                        if !force_draw {
                            if p_rdr <= 1 {
                                draw = true;
                            } else {
                                drop += 1;
                            }
                        }
                    }

                    let mut first_offset = 0;
                    if draw {
                        fps += 1;

                        // the slow part
                        {
                            let screen = screen.read();
                            let last_screen = last_screen.read();
                            first_offset = screen.first_offset.unwrap_or(0);

                            draw_view(&last_screen, &screen, &mut stdout);
                        }
                        last_screen = screen;
                    }

                    if (start - fps_t0).as_millis() >= 1000 {
                        if debug {
                            eprintln!(
                                 "DRAW: crossterm | offset {:?} | req {} | fps {} | drop {} | p_rdr {} | p_input {}",
                                 first_offset,
                                 draw_req,
                                 fps,
                                 drop,
                                 p_rdr,
                                 p_input
                             );
                        }

                        fps = 0;
                        drop = 0;
                        draw_req = 0;
                        fps_t0 = Instant::now();
                    }

                    let ts_now = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

                    if debug {
                        eprintln!(
                            "CROSSTERM: input latency: ts_now {} - input_ts {} = {}",
                            ts_now,
                            msg.input_ts,
                            ts_now - msg.input_ts
                        );
                    }

                    crate::core::event::pending_render_event_dec(1);
                }

                _ => {}
            }

            if debug {
                let ts1 = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();
                eprintln!("CROSSTERM: event handling time {}", ts1 - ts);
            }
        }
    }

    if crate::core::bench_to_eof() {
        /* Terminate crossterm: no clear */
        execute!(
            stdout,
            SetAttribute(Attribute::Reset),
            DisableMouseCapture,
            Show
        )?;
    } else {
        /* Terminate crossterm */
        execute!(
            stdout,
            SetAttribute(Attribute::Reset),
            Clear(ClearType::All),
            DisableMouseCapture,
            Show
        )?;

        execute!(stdout, LeaveAlternateScreen,)?;
    }

    if supports_keyboard_enhancement {
        execute!(stdout, PopKeyboardEnhancementFlags)?;
    }

    crossterm::terminal::disable_raw_mode()?;

    Ok(())
}

/*
    TODO(ceg):
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idiomatic    func()? style
*/
fn draw_view(last_screen: &Screen, screen: &Screen, stdout: &mut std::io::StdoutLock) {
    if true {
        let _ = draw_screen(last_screen, screen, stdout);
    } else {
        let _ = draw_screen_dumb(screen, stdout);
    }
}

#[derive(Debug, Clone)]
enum ScreenOp {
    MoveTo(u16, u16),
    SetFgColor(u8, u8, u8),
    SetBgColor(u8, u8, u8),
    SetNormal,
    SetInverse,
    SetNoInverse,
    SetBold,
    SetNoBold,
    PrintText(char),
}

fn draw_screen_dumb(
    screen: &Screen,
    stdout: &mut std::io::StdoutLock,
) -> Result<(), std::io::Error> {
    // queue!(stdout, ResetColor)?;
    // queue!(stdout, Clear(ClearType::All))?;

    let mut ops = vec![];

    // current Brush/Pen
    let mut prev_fg = (0, 0, 0);
    let mut prev_bg = (0, 0, 0);
    let mut is_inverse = false;
    let mut is_bold = false;

    // reset Style
    queue!(stdout, SetAttribute(Attribute::NoReverse))?;

    for li in 0..screen.height() {
        ops.push(ScreenOp::MoveTo(0, li as u16));

        // TODO(ceg): fill len.len()..screen.width()
        let line = screen.get_line(li).unwrap();

        for (c, cell) in line.iter().enumerate() {
            let cpi = cell.cpi;

            // dbg_println!("RENDER Y={} X={} : cpi {:?}", li, c, cpi);
            if cpi.skip_render {
                ops.push(ScreenOp::MoveTo(c as u16 + 1, li as u16));
                continue;
            }

            // fg color
            if prev_fg != cpi.style.color {
                ops.push(ScreenOp::SetFgColor(
                    cpi.style.color.0,
                    cpi.style.color.1,
                    cpi.style.color.2,
                ));
            }
            prev_fg = cpi.style.color;

            // bg color
            if prev_bg != cpi.style.bg_color {
                ops.push(ScreenOp::SetBgColor(
                    cpi.style.bg_color.0,
                    cpi.style.bg_color.1,
                    cpi.style.bg_color.2,
                ));
            }
            prev_bg = cpi.style.bg_color;

            // inverse
            if cpi.style.is_inverse != is_inverse {
                if cpi.style.is_inverse {
                    ops.push(ScreenOp::SetInverse);
                } else {
                    ops.push(ScreenOp::SetNoInverse);
                }
            }
            is_inverse = cpi.style.is_inverse;

            // bold
            if cpi.style.is_bold != is_bold {
                if cpi.style.is_bold {
                    ops.push(ScreenOp::SetBold);
                } else {
                    ops.push(ScreenOp::SetNoBold);
                }
            }
            is_bold = cpi.style.is_bold;

            ops.push(ScreenOp::PrintText(cpi.displayed_cp));
        }
    }

    dbg_println!("NB screen (dumb) ops {}\r", ops.len());

    for op in ops {
        match op {
            ScreenOp::MoveTo(x, y) => queue!(stdout, MoveTo(x, y))?,
            ScreenOp::SetFgColor(r, g, b) => {
                queue!(stdout, SetForegroundColor(Color::Rgb { r, g, b }))?
            }
            ScreenOp::SetBgColor(r, g, b) => {
                queue!(stdout, SetBackgroundColor(Color::Rgb { r, g, b }))?
            }
            ScreenOp::SetNormal => {
                queue!(stdout, SetAttribute(Attribute::NoReverse))?;
                queue!(stdout, SetAttribute(Attribute::NormalIntensity))?
            }
            ScreenOp::SetInverse => queue!(stdout, SetAttribute(Attribute::Reverse))?,
            ScreenOp::SetNoInverse => queue!(stdout, SetAttribute(Attribute::NoReverse))?,
            ScreenOp::SetBold => queue!(stdout, SetAttribute(Attribute::Bold))?,
            ScreenOp::SetNoBold => queue!(stdout, SetAttribute(Attribute::NormalIntensity))?,

            ScreenOp::PrintText(c) => queue!(stdout, Print(c))?,
        }
    }

    // NB: Always flush screen updates
    stdout.flush()?;

    Ok(())
}

fn screen_changed(screen0: &Screen, screen1: &Screen) -> bool {
    let o = screen0.first_offset != screen1.first_offset;
    let w = screen0.width() != screen1.width();
    let h = screen0.height() != screen1.height();
    o || w || h
}

fn screen_width_change(screen0: &Screen, screen1: &Screen) -> bool {
    screen0.width() != screen1.width()
}

fn screen_height_change(screen0: &Screen, screen1: &Screen) -> bool {
    screen0.height() != screen1.height()
}

fn cpis_have_same_style(a: &CodepointInfo, b: &CodepointInfo) -> bool {
    *a == *b
}

/*
   TODO(ceg):
     if width || height change -> clear redraw all
*/
fn draw_screen(
    last_screen: &Screen,
    screen: &Screen,
    stdout: &mut std::io::StdoutLock,
) -> Result<(), std::io::Error> {
    let _screen_change = screen_changed(last_screen, screen);
    let width_change = screen_width_change(last_screen, screen);
    let height_change = screen_height_change(last_screen, screen);

    if width_change || height_change {
        let _ = draw_screen_dumb(screen, stdout);
        return Ok(());
    }

    // current style
    let width = screen.width();
    let height = screen.height();

    let t0 = Instant::now();

    // reset Style
    queue!(stdout, SetAttribute(Attribute::NoReverse))?;

    let mut ops = vec![];

    // current Brush/Pen
    let mut prev_bg = (0, 0, 0);
    let mut prev_fg = (0, 0, 0);

    let mut is_inverse = false;
    let mut is_bold = false;

    let mut l = 0;
    while l < height {
        let line = screen.get_line(l);
        if line.is_none() {
            panic!("");
        }

        let line = line.unwrap();

        let mut c = 0;
        while c < width {
            let prev_c = c;

            // let mut have_diff = false;
            // get next diff
            if true {
                let prev_line = last_screen.get_line(l).unwrap();

                while c < width {
                    let prev_screen_cpi = &prev_line[c].cpi;
                    let cpi = &line[c].cpi;

                    if !cpis_have_same_style(cpi, prev_screen_cpi) {
                        // diff found stop @ c
                        // debug_cpi.style.is_bold = true;
                        //debug_cpi.style.bg_color.0 = 255;
                        //debug_cpi.displayed_cp = 'X';
                        // have_diff = true;
                        break;
                    } else {
                        c += 1;
                        // debug_cpi.style.bg_color.1 = 255;
                        // debug_cpi.style.is_inverse = true;
                    }
                }

                // no diff ?
                if c >= width {
                    // end-of-line stop
                    break;
                }

                // adjust line,column
            }

            if c != prev_c {
                ops.push(ScreenOp::MoveTo(c as u16, l as u16));
            } else if c == 0 {
                ops.push(ScreenOp::MoveTo(0, l as u16));
            }

            let cpi = &line[c].cpi;
            if cpi.skip_render {
                c += 1;
                continue;
            }

            // inverse
            if cpi.style.is_inverse != is_inverse {
                if cpi.style.is_inverse {
                    ops.push(ScreenOp::SetInverse);
                } else {
                    ops.push(ScreenOp::SetNormal);
                }
            }
            is_inverse = cpi.style.is_inverse;

            // bold
            if cpi.style.is_bold != is_bold {
                if cpi.style.is_bold {
                    ops.push(ScreenOp::SetBold);
                } else {
                    ops.push(ScreenOp::SetNoBold);
                }
            }
            is_bold = cpi.style.is_bold;

            // fg color
            if prev_fg != cpi.style.color {
                ops.push(ScreenOp::SetFgColor(
                    cpi.style.color.0,
                    cpi.style.color.1,
                    cpi.style.color.2,
                ));
            }
            prev_fg = cpi.style.color;

            // bg color
            if prev_bg != cpi.style.bg_color {
                ops.push(ScreenOp::SetBgColor(
                    cpi.style.bg_color.0,
                    cpi.style.bg_color.1,
                    cpi.style.bg_color.2,
                ));
            }

            prev_bg = cpi.style.bg_color;

            ops.push(ScreenOp::PrintText(cpi.displayed_cp));

            c += 1;
        }

        l += 1;
    }

    for op in ops {
        match op {
            ScreenOp::MoveTo(x, y) => queue!(stdout, MoveTo(x, y))?,
            ScreenOp::SetFgColor(r, g, b) => {
                queue!(stdout, SetForegroundColor(Color::Rgb { r, g, b }))?
            }
            ScreenOp::SetBgColor(r, g, b) => {
                queue!(stdout, SetBackgroundColor(Color::Rgb { r, g, b }))?
            }
            ScreenOp::SetNormal => {
                queue!(stdout, SetAttribute(Attribute::NoReverse))?;
                queue!(stdout, SetAttribute(Attribute::NormalIntensity))?
            }
            ScreenOp::SetInverse => queue!(stdout, SetAttribute(Attribute::Reverse))?,
            ScreenOp::SetNoInverse => queue!(stdout, SetAttribute(Attribute::NoReverse))?,
            ScreenOp::SetBold => queue!(stdout, SetAttribute(Attribute::Bold))?,
            ScreenOp::SetNoBold => queue!(stdout, SetAttribute(Attribute::NormalIntensity))?,

            ScreenOp::PrintText(c) => queue!(stdout, Print(c))?,
        }
    }

    let t1 = Instant::now();

    if false {
        queue!(stdout, MoveTo(0, 0))?;
        queue!(stdout, SetAttribute(Attribute::Reset))?;
        queue!(stdout, Clear(ClearType::CurrentLine))?;
        let debug_str = format!("debug: time to render {}", (t1 - t0).as_micros());
        queue!(stdout, Print(debug_str))?;
    }

    // NB: Always flush screen updates
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
        // ...event::MouseButton::WheelUp => 3,
        // ...event::MouseButton::WheelDown => 4,
    } //
}

fn translate_crossterm_event(evt: ::crossterm::event::Event) -> InputEvent {
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

            ::crossterm::event::KeyCode::Pause => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::Menu => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::KeypadBegin => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::Media(_) => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::Modifier(_) => {
                return InputEvent::NoInputEvent;
            }

            ::crossterm::event::KeyCode::CapsLock => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::ScrollLock => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::NumLock => {
                return InputEvent::NoInputEvent;
            }
            ::crossterm::event::KeyCode::PrintScreen => {
                return InputEvent::NoInputEvent;
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
                // TODO(ceg): no Drag event in the editor yet ?
                // TODO(ceg): filter dragged button
                // return InputEvent::NoInputEvent;

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

            ::crossterm::event::MouseEventKind::ScrollLeft => {
                panic!("");
                return InputEvent::NoInputEvent;
            }

            ::crossterm::event::MouseEventKind::ScrollRight => {
                panic!("");
                return InputEvent::NoInputEvent;
            }
        },

        ::crossterm::event::Event::Resize(width, height) => {
            // println!("New size {}x{}", width, height)
            // TODO(ceg): not really an input
            return InputEvent::RefreshUi {
                width: width as usize,
                height: height as usize,
            };
        }

        ::crossterm::event::Event::FocusGained => {}
        ::crossterm::event::Event::FocusLost => {}

        ::crossterm::event::Event::Paste(s) => {
            let v: Vec<char> = s
                .chars()
                .map(|c| if c == '\r' { '\n' } else { c }) // TODO: move this to text mode and use Paste(s)
                .collect();

            return InputEvent::KeyPress {
                mods: KeyModifiers::new(),
                key: Key::UnicodeArray(v),
            };

            return InputEvent::Paste(s);
        }
    }

    InputEvent::NoInputEvent
}

fn send_input_events(
    accum: Vec<InputEvent>,
    ts: u128,
    tx: &Sender<Message>,
    _ui_tx: &Sender<Message>,
) {
    let mut v = Vec::<InputEvent>::new();

    // merge consecutive characters as "array" of chars
    let mut codepoints = Vec::<char>::new();

    if accum.len() == 1 {
        match accum[0] {
            InputEvent::RefreshUi { width, height } => {
                let msg = Message::new(0, 0, ts, Event::UpdateView { width, height });

                // ui_tx.send(msg).unwrap_or(()); ?

                // send to core
                crate::core::event::pending_input_event_inc(1);
                tx.send(msg).unwrap_or(());
            }

            _ => {
                // send
                let msg = Message::new(0, 0, ts, Event::Input { events: accum });
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
                new_width = width;
                new_height = height;
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
                codepoints.push(c);
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
        let msg = Message::new(
            0,
            0,
            ts,
            Event::UpdateView {
                width: new_width,
                height: new_height,
            },
        );
        crate::core::event::pending_input_event_inc(1);
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
        let msg = Message::new(0, 0, ts, Event::Input { events: v });
        crate::core::event::pending_input_event_inc(ev_count);
        tx.send(msg).unwrap_or(());
    }
}

fn get_input_events(
    tx: &Sender<Message>,
    ui_tx: &Sender<Message>,
    force_input: bool,
) -> Result<(), std::io::Error> {
    let mut accum = Vec::<InputEvent>::with_capacity(255);
    let mut wait_ms = 60_000;
    let max_wait_ms = 150;
    let min_wait_ms = 1;

    let mut start = Instant::now();
    let mut prev_len = 0;

    let mut count = 0;

    // accumulate events up to max_wait_ms millisecond
    loop {
        if ::crossterm::event::poll(Duration::from_millis(wait_ms))? {
            if let Ok(cross_evt) = ::crossterm::event::read() {
                prev_len = accum.len();
                let evt = translate_crossterm_event(cross_evt);
                accum.push(evt);
                if force_input {
                    break;
                }
            }
        }

        wait_ms = min_wait_ms;

        count += 1;
        if count == 1 {
            // delay flush of 1st input event (min_wait_ms)
            // real start
            start = Instant::now();
            continue;
        }

        // count >= 2
        let d = start.elapsed();
        if d >= Duration::from_millis(max_wait_ms) {
            break;
        }

        if prev_len == accum.len() {
            // no new event received -> flush
            break;
        }
        prev_len = accum.len();
    }

    // TODO(ceg): --limit-input-rate
    let p_input = crate::core::event::pending_input_event_count();
    if p_input > 16 && !force_input {
        return Ok(());
    }

    let ts = crate::core::BOOT_TIME.elapsed().unwrap().as_millis();

    if !accum.is_empty() {
        send_input_events(accum, ts, tx, ui_tx);
    }

    Ok(())
}
