use crate::dbg_println;

use std::io::{stdout, Write};

use crossterm::{
    cursor::{Hide, MoveTo, Show},
    event,
    event::{DisableMouseCapture, EnableMouseCapture},
    queue,
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

use parking_lot::RwLock;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::sync::Arc;

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

fn stdin_thread(core_tx: &Sender<EventMessage>, ui_tx: &Sender<EventMessage>) {
    // TODO(ceg): generate_test from logs grep | awk >>
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
    let startup = Instant::now();
    let mut draw_req = 0;
    let mut fps = 0;
    let mut fps_t0 = Instant::now();

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

    // ui ctx : TODO move to struct UiCtx
    let mut last_screen = Arc::new(RwLock::new(Box::new(Screen::new(0, 0))));

    let stdout = stdout();
    let mut stdout = stdout.lock();

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
        Hide,
        SetAttribute(Attribute::Reset),
        SetBackgroundColor(bg_color),
        SetForegroundColor(color),
        Clear(ClearType::All)
    )?;

    crossterm::terminal::enable_raw_mode()?;

    // first request
    // check terminal size
    let (width, height) = crossterm::terminal::size().ok().unwrap();
    let msg = EventMessage::new(
        get_next_seq(&mut seq),
        Event::UpdateView {
            width: width as usize,
            height: height as usize,
        },
    );
    crate::core::event::pending_input_event_inc(1);
    core_tx.send(msg).unwrap_or(()); // if removed the 1st screen is not displayed

    loop {
        if let Ok(evt) = ui_rx.recv() {
            match evt.event {
                Event::ApplicationQuitEvent => {
                    let msg =
                        EventMessage::new(get_next_seq(&mut seq), Event::ApplicationQuitEvent);
                    crate::core::event::pending_input_event_inc(1);
                    core_tx.send(msg).unwrap_or(());
                    break;
                }

                UpdateView { width, height } => {
                    let msg = EventMessage::new(
                        get_next_seq(&mut seq),
                        Event::UpdateView { width, height },
                    );
                    crate::core::event::pending_input_event_inc(1);
                    core_tx.send(msg).unwrap_or(());
                }

                Draw { screen, time: _ } => {
                    draw_req += 1;

                    let start = Instant::now();
                    crate::core::event::pending_render_event_dec(1);
                    let p_rdr = crate::core::event::pending_render_event_count();
                    let p_input = crate::core::event::pending_input_event_count();

                    if crate::core::bench_to_eof() {
                        if (start - fps_t0).as_millis() >= 1000 {
                            let screen = screen.read();

                            eprintln!(
                                "DRAW: crossterm | time {}| offset {:?} | req {} | fps {} | p_rdr {} | p_input {}",
                                start.duration_since(startup).as_millis(),
                                screen.first_offset,
                                draw_req,
                                fps,
                                p_rdr,
                                p_input
                            );

                            fps = 0;
                            draw_req = 0;
                            fps_t0 = start;
                        }
                    }

                    let mut draw = false;

                    if p_rdr < 1 {
                        draw = true;
                    }

                    if crate::core::no_ui_render() {
                        draw = false;
                        fps += 1;
                    }

                    if draw {
                        fps += 1;

                        // the slow part
                        {
                            let mut screen = screen.write();
                            let mut last_screen = last_screen.write();
                            draw_view(&mut last_screen, &mut screen, &mut stdout);
                        }
                        last_screen = screen;
                    }

                    if false {
                        let p_rdr = crate::core::event::pending_render_event_count();

                        let end = Instant::now();

                        dbg_println!("DRAW: crossterm : time spent to draw view = {} µs | fps: {}| p_input {}|p_rdr {}| draw:{}\r",
                        (end - start).as_micros(),
                        fps,
                        p_input,
                        p_rdr, draw
                    );
                    }
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
    TODO(ceg):
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idiomatic    func()? style
*/
fn draw_view(
    mut last_screen: &mut Screen,
    mut screen: &mut Screen,
    mut stdout: &mut std::io::StdoutLock,
) {
    if true {
        let _ = draw_screen(&mut last_screen, &mut screen, &mut stdout);
    } else {
        let _ = draw_screen_dumb(&screen, &mut stdout);
    }
}

enum ScreenOp {
    MoveTo(u16, u16),
    SetFgColor(u8, u8, u8),
    SetBgColor(u8, u8, u8),
    SetNormal,
    SetInverse,
    PrintText(char),
}

fn draw_screen_dumb(screen: &Screen, stdout: &mut std::io::StdoutLock) -> Result<()> {
    // queue!(stdout, ResetColor)?;
    // queue!(stdout, Clear(ClearType::All))?;

    let mut ops = vec![];

    // current Brush/Pen
    let mut prev_fg = None;
    let mut prev_bg = None;
    let mut is_inverse = false;

    // reset Style
    queue!(stdout, SetAttribute(Attribute::NoReverse))?;

    for li in 0..screen.height() {
        ops.push(ScreenOp::MoveTo(0, li as u16));

        // TODO(ceg): fill len.len()..screen.width()
        let line = screen.get_line(li).unwrap();

        for c in 0..line.len() {
            let cpi = &line[c].cpi;

            // dbg_println!("RENDER Y={} X={} : cpi {:?}", li, c, cpi);
            if cpi.skip_render {
                // ops.push(ScreenOp::MoveTo(c as u16 + 1, li as u16));
                // continue;
            }

            // fg color
            let color = Color::Rgb {
                r: cpi.style.color.0,
                g: cpi.style.color.1,
                b: cpi.style.color.2,
            };
            if prev_fg.is_none() {
                ops.push(ScreenOp::SetFgColor(
                    cpi.style.color.0,
                    cpi.style.color.1,
                    cpi.style.color.2,
                ));
            } else {
                if *prev_fg.as_ref().unwrap() != color {
                    ops.push(ScreenOp::SetFgColor(
                        cpi.style.color.0,
                        cpi.style.color.1,
                        cpi.style.color.2,
                    ));
                }
            }
            prev_fg = Some(color);

            // bg color
            let bg_color = Color::Rgb {
                r: cpi.style.bg_color.0,
                g: cpi.style.bg_color.1,
                b: cpi.style.bg_color.2,
            };

            if prev_bg.is_none() {
                ops.push(ScreenOp::SetBgColor(
                    cpi.style.bg_color.0,
                    cpi.style.bg_color.1,
                    cpi.style.bg_color.2,
                ));
            } else {
                if *prev_bg.as_ref().unwrap() != bg_color {
                    ops.push(ScreenOp::SetBgColor(
                        cpi.style.bg_color.0,
                        cpi.style.bg_color.1,
                        cpi.style.bg_color.2,
                    ));
                }
            }

            prev_bg = Some(bg_color);

            // inverse
            if cpi.style.is_inverse != is_inverse {
                if cpi.style.is_inverse {
                    ops.push(ScreenOp::SetInverse);
                } else {
                    ops.push(ScreenOp::SetNormal);
                }
            }
            is_inverse = cpi.style.is_inverse;

            ops.push(ScreenOp::PrintText(cpi.displayed_cp));
        }
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
            ScreenOp::SetNormal => queue!(stdout, SetAttribute(Attribute::NoReverse))?,
            ScreenOp::SetInverse => queue!(stdout, SetAttribute(Attribute::Reverse))?,
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
    last_screen: &mut Screen,
    screen: &mut Screen,
    mut stdout: &mut std::io::StdoutLock,
) -> Result<()> {
    let _screen_change = screen_changed(&last_screen, &screen);
    let width_change = screen_width_change(&last_screen, &screen);
    let height_change = screen_height_change(&last_screen, &screen);

    if width_change || height_change {
        let _ = draw_screen_dumb(&screen, &mut stdout);
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
    let mut prev_fg = None;
    let mut prev_bg = None;
    let mut is_inverse = false;

    let mut l = 0;
    while l < height {
        let line = screen.get_line_mut(l);
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

            // fg color
            let color = Color::Rgb {
                r: cpi.style.color.0,
                g: cpi.style.color.1,
                b: cpi.style.color.2,
            };
            if prev_fg.is_none() {
                ops.push(ScreenOp::SetFgColor(
                    cpi.style.color.0,
                    cpi.style.color.1,
                    cpi.style.color.2,
                ));
            } else {
                if *prev_fg.as_ref().unwrap() != color {
                    ops.push(ScreenOp::SetFgColor(
                        cpi.style.color.0,
                        cpi.style.color.1,
                        cpi.style.color.2,
                    ));
                }
            }
            prev_fg = Some(color);

            // bg color
            let bg_color = Color::Rgb {
                r: cpi.style.bg_color.0,
                g: cpi.style.bg_color.1,
                b: cpi.style.bg_color.2,
            };

            if prev_bg.is_none() {
                ops.push(ScreenOp::SetBgColor(
                    cpi.style.bg_color.0,
                    cpi.style.bg_color.1,
                    cpi.style.bg_color.2,
                ));
            } else {
                if *prev_bg.as_ref().unwrap() != bg_color {
                    ops.push(ScreenOp::SetBgColor(
                        cpi.style.bg_color.0,
                        cpi.style.bg_color.1,
                        cpi.style.bg_color.2,
                    ));
                }
            }

            prev_bg = Some(bg_color);

            ops.push(ScreenOp::PrintText(cpi.displayed_cp));

            c += 1;
        }

        l += 1;
    }

    dbg_println!("NB ops {}\r", ops.len());

    for op in ops {
        match op {
            ScreenOp::MoveTo(x, y) => queue!(stdout, MoveTo(x, y))?,
            ScreenOp::SetFgColor(r, g, b) => {
                queue!(stdout, SetForegroundColor(Color::Rgb { r, g, b }))?
            }
            ScreenOp::SetBgColor(r, g, b) => {
                queue!(stdout, SetBackgroundColor(Color::Rgb { r, g, b }))?
            }
            ScreenOp::SetNormal => queue!(stdout, SetAttribute(Attribute::NoReverse))?,
            ScreenOp::SetInverse => queue!(stdout, SetAttribute(Attribute::Reverse))?,
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

fn translate_crossterm_event(
    evt: ::crossterm::event::Event,
    pending_resize: &mut bool,
) -> InputEvent {
    // translate crossterm event
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
                // TODO(ceg): no Drag event in the editor yet ?
                // TODO(ceg): filter dragged button

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
            // TODO(ceg): not really an input
            *pending_resize = true;
            return InputEvent::RefreshUi {
                width: width as usize,
                height: height as usize,
            };
        }

        ::crossterm::event::Event::Terminate => {
            // TODO(ceg): not really an input
        }
    }

    InputEvent::NoInputEvent
}

fn send_input_events(
    accum: Vec<InputEvent>,
    tx: &Sender<EventMessage>,
    _ui_tx: &Sender<EventMessage>,
) {
    let mut v = Vec::<InputEvent>::new();

    // merge consecutive characters as "array" of chars
    let mut codepoints = Vec::<char>::new();

    if accum.len() == 1 {
        match accum[0] {
            InputEvent::RefreshUi { width, height } => {
                let msg = EventMessage::new(0, Event::UpdateView { width, height });

                // ui_tx.send(msg).unwrap_or(()); ?

                // send to core
                crate::core::event::pending_input_event_inc(1);
                tx.send(msg).unwrap_or(());
            }

            _ => {
                // send
                let msg = EventMessage::new(0, Event::Input { events: accum });
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
        let msg = EventMessage::new(
            0,
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
        let msg = EventMessage::new(0, Event::Input { events: v });
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
    let mut min_wait_ms = 1;

    let mut start = Instant::now();
    let mut prev_ev_time = start;

    let mut count = 0;
    let mut pending_resize = false;

    // accumulate events up to 1 millisecond
    loop {
        if ::crossterm::event::poll(Duration::from_millis(wait_ms))? {
            if let Ok(cross_evt) = ::crossterm::event::read() {
                prev_ev_time = Instant::now();
                let evt = translate_crossterm_event(cross_evt, &mut pending_resize);
                accum.push(evt);
                if pending_resize {
                    min_wait_ms = 16; // wait for other resize events
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

    // TODO(ceg): --limit-input-rate
    let p_input = crate::core::event::pending_input_event_count();
    if p_input > 0 {
        return Ok(());
    }

    if accum.is_empty() {
        return Ok(());
    }

    send_input_events(accum, tx, ui_tx);

    Ok(())
}
