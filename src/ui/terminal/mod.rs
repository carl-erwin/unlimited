//
extern crate termion;


//
use self::termion::screen::AlternateScreen;

use self::termion::input::{TermRead, MouseTerminal};
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;

use std::io::{self, Write, stdin, Stdout};

//
use core::view::View;
use core::screen::Screen;
use core::codepointinfo::CodepointInfo;

use core::event::InputEvent;
use core::event::Key;


use core::editor::Editor;


use core::codec::text::utf8;
use core::codec::text::utf8::{UTF8_ACCEPT, UTF8_REJECT};

use core::codec::text::u32_to_char;


//
struct UiState {
    keys: Vec<InputEvent>,
    quit: bool,
    status: String,
    display_status: bool,
    display_view: bool,
    vid: u64,
    nb_view: usize,
    last_offset: u64,
    mark_offset: u64,
}

impl UiState {
    fn new() -> UiState {
        UiState {
            keys: Vec::new(),
            quit: false,
            status: String::new(),
            display_status: true,
            display_view: true,
            vid: 0,
            nb_view: 0,
            last_offset: 0,
            mark_offset: 0,
        }
    }
}

pub fn main_loop(mut editor: &mut Editor) {

    let mut ui_state = UiState::new();

    let (width, height) = terminal_size().unwrap();

    setup_views(editor, width as usize, height as usize);

    //
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
    stdout.flush().unwrap();

    while !ui_state.quit {

        ui_state.nb_view = editor.view_map.len();
        let mut view = editor.view_map.get_mut(&ui_state.vid);

        let status_line_y = height;

        if ui_state.display_view == true {
            draw_view(&mut ui_state, &mut view.as_mut().unwrap(), &mut stdout);
        }

        if ui_state.display_status == true {
            display_status_line(&ui_state,
                                &mut view.as_mut().unwrap(),
                                status_line_y,
                                width,
                                &mut stdout);
        }

        let evt = get_input_event(&mut ui_state);

        process_input_events(&mut ui_state, &mut view.as_mut().unwrap(), evt);
    }

    // quit
    // clear, restore cursor
    write!(stdout, "{}{}", termion::clear::All, termion::cursor::Show).unwrap();
    stdout.flush().unwrap();
}


fn setup_views(editor: &mut Editor, width: usize, height: usize) {

    let mut views = Vec::new();

    let mut vid = 0;

    for (_, b) in &editor.document_map {

        let view = View::new(vid,
                             0 as u64,
                             width as usize,
                             height as usize,
                             Some(b.clone()));
        views.push(view);
        vid += 1;
    }

    for view in views {
        &editor.view_map.insert(view.id, Box::new(view));
    }
}


fn screen_putstr(mut screen: &mut Screen, s: &str) -> bool {

    let v: Vec<char> = s.chars().collect();
    for c in &v {
        let ok = screen_putchar(&mut screen, *c, 0xffffffffffffffff);
        if ok == false {
            return false;
        }
    }

    true
}


//
fn filter_codepoint(c: char, offset: u64) -> CodepointInfo {

    let displayed_cp = match c {
        '\n' | '\t' => ' ',
        _ => c,
    };

    CodepointInfo {
        cp: c,
        displayed_cp: displayed_cp,
        offset: offset,
        is_selected: false,
    }
}


fn screen_putchar(mut screen: &mut Screen, c: char, offset: u64) -> bool {
    let (ok, _) = screen.push(filter_codepoint(c, offset));
    ok
}


fn decode_slice_to_vec(data: &[u8], base_offset: u64, max_cpi: usize) -> (Vec<CodepointInfo>, u64) {

    let mut offset = base_offset;
    let mut state: u32 = 0;
    let mut cp_val: u32 = 0;
    let mut cp_start_offset = base_offset;

    let mut vec = Vec::with_capacity(max_cpi);

    for b in data {
        state = utf8::decode_byte(&mut state, *b, &mut cp_val);
        match state {
            UTF8_ACCEPT => {
                let cp = u32_to_char(cp_val);
                vec.push(filter_codepoint(cp, cp_start_offset));
                cp_start_offset = offset + 1;

                // reset state
                cp_val = 0;
                state = UTF8_ACCEPT;
            }

            UTF8_REJECT => {
                for i in cp_start_offset..offset + 1 {
                    vec.push(filter_codepoint('�', i));
                }
                cp_start_offset = offset + 1;

                // reset state
                cp_val = 0;
                state = UTF8_ACCEPT;
            }

            _ => { /* intermediate state */ }
        }

        offset += 1;
        if vec.len() == max_cpi {
            break;
        }
    }

    (vec, offset)
}

fn decode_slice_to_screen(data: &[u8], base_offset: u64, mut screen: &mut Screen) -> u64 {

    let max_cpi = screen.width * screen.height;
    let (vec, last_offset) = decode_slice_to_vec(data, base_offset, max_cpi);

    for cpi in &vec {
        let (ok, _) = screen.push(cpi.clone());
        if ok == false {
            break;
        }
    }

    last_offset
}


fn fill_screen(mut ui_state: &mut UiState, mut view: &mut View) {

    match view.document {

        Some(ref buf) => {

            let mut screen = &mut view.screen;

            screen.clear();

            // render first screen line
            {
                let s = " unlimitED! v0.0.1\n\n";
                screen_putstr(&mut screen, &s);
                let mut line = screen.get_mut_line(0).unwrap();
                for c in 0..line.width {
                    let mut cpi = line.get_mut_cpi(c).unwrap();
                    cpi.is_selected = true;
                }
            }

            let data = &buf.borrow().buffer.data;
            let len = data.len();

            view.end_offset = decode_slice_to_screen(&data[0..len], view.start_offset, &mut screen);

            if view.end_offset == buf.borrow().buffer.size as u64 {
                screen.push(CodepointInfo {
                                cp: ' ',
                                displayed_cp: ' ',
                                offset: view.end_offset,
                                is_selected: false,
                            });
            }

            ui_state.last_offset = view.end_offset;

            // brute force for now
            for m in &view.moving_marks {

                // TODO: screen.find_line_by_offset(m.offset) -> Option<&mut Line>
                if m.offset >= view.start_offset && m.offset <= view.end_offset {
                    for l in 0..screen.height {
                        let line = screen.get_mut_line(l).unwrap();
                        for c in 0..line.used {
                            let mut cpi = line.get_mut_cpi(c).unwrap();

                            if cpi.offset > m.offset {
                                break;
                            }

                            if cpi.offset == m.offset {
                                cpi.is_selected = true;
                                ui_state.mark_offset = m.offset;
                            }
                        }
                    }
                }
            }
        }
        None => {}
    }
}


fn draw_screen(screen: &mut Screen, mut stdout: &mut Stdout) {

    write!(stdout, "{}", termion::cursor::Goto(1, 1)).unwrap();
    write!(stdout, "{}", termion::style::Reset).unwrap();

    for l in 0..screen.height {

        terminal_cursor_to(&mut stdout, 1, (l + 1) as u16);

        let line = screen.get_line(l).unwrap();

        for c in 0..line.width {

            let cpi = line.get_cpi(c).unwrap();

            if cpi.is_selected == true {
                write!(stdout, "{}", termion::style::Invert).unwrap();
            }

            write!(stdout, "{}", cpi.displayed_cp).unwrap();
            write!(stdout, "{}", termion::style::Reset).unwrap();
        }

        /*
        for _ in line.used..line.width {
            write!(stdout, " ").unwrap();
        }
        */
    }

    stdout.flush().unwrap();
}


/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(mut ui_state: &mut UiState, mut view: &mut View, mut stdout: &mut Stdout) {

    fill_screen(&mut ui_state, &mut view);
    draw_screen(&mut view.screen, &mut stdout);
}

fn terminal_clear_current_line(mut stdout: &mut Stdout, line_width: u16) {
    for _ in 0..line_width {
        write!(stdout, " ").unwrap();
    }
}

fn terminal_cursor_to(mut stdout: &mut Stdout, x: u16, y: u16) {
    write!(stdout, "{}", termion::cursor::Goto(x, y)).unwrap();
}



fn get_input_event(ui_state: &mut UiState) -> InputEvent {

    for evt in stdin().events() {

        let evt = evt.unwrap();

        // translate termion event
        match evt {

            self::termion::event::Event::Key(k) => {
                match k {

                    self::termion::event::Key::Ctrl('c') => {
                        ui_state.status = format!("Ctrl-c");

                        return InputEvent::KeyPress {
                                   ctrl: true,
                                   alt: false,
                                   shift: false,
                                   key: Key::UNICODE('c'),
                               };
                    }

                    self::termion::event::Key::Char('\n') => {
                        ui_state.status = format!("{}", "<newline>");

                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::UNICODE('\n'),
                               };
                    }

                    self::termion::event::Key::Char(c) => {
                        ui_state.status = format!("{}", c);

                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::UNICODE(c),
                               };
                    }

                    self::termion::event::Key::Alt(c) => {
                        ui_state.status = format!("Alt-{}", c);

                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: true,
                                   shift: false,
                                   key: Key::UNICODE(c),
                               };
                    }

                    self::termion::event::Key::Ctrl(c) => {
                        ui_state.status = format!("Ctrl-{}", c);

                        return InputEvent::KeyPress {
                                   ctrl: true,
                                   alt: false,
                                   shift: false,
                                   key: Key::UNICODE(c),
                               };
                    }

                    self::termion::event::Key::F(1) => {
                        if ui_state.vid > 0 {
                            ui_state.vid -= 1;
                        }
                        break;
                    }

                    self::termion::event::Key::F(2) => {
                        ui_state.vid = ::std::cmp::min(ui_state.vid + 1,
                                                       (ui_state.nb_view - 1) as u64);
                        break;
                    }

                    self::termion::event::Key::F(f) => ui_state.status = format!("F{:?}", f),

                    self::termion::event::Key::Left => {
                        ui_state.status = format!("<left>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Left,
                               };
                    }
                    self::termion::event::Key::Right => {
                        ui_state.status = format!("<right>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Right,
                               };
                    }
                    self::termion::event::Key::Up => {
                        ui_state.status = format!("<up>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Up,
                               };
                    }
                    self::termion::event::Key::Down => {
                        ui_state.status = format!("<down>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Down,
                               };
                    }
                    self::termion::event::Key::Backspace => {
                        ui_state.status = format!("<backspc>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::BackSpace,
                               };
                    }
                    self::termion::event::Key::Home => {
                        ui_state.status = format!("<Home>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Home,
                               };

                    }
                    self::termion::event::Key::End => {
                        ui_state.status = format!("<End>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::End,
                               };

                    }
                    self::termion::event::Key::PageUp => {
                        ui_state.status = format!("<PageUp>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::PageUp,
                               };
                    }
                    self::termion::event::Key::PageDown => {
                        ui_state.status = format!("<PageDown>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::PageDown,
                               };
                    }
                    self::termion::event::Key::Delete => {
                        ui_state.status = format!("<Delete>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Delete,
                               };
                    }
                    self::termion::event::Key::Insert => {
                        ui_state.status = format!("<Insert>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Insert,
                               };
                    }
                    self::termion::event::Key::Esc => {
                        ui_state.status = format!("<Esc>");
                        return InputEvent::KeyPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   key: Key::Escape,
                               };
                    }
                    _ => ui_state.status = format!("Other"),
                }
            }

            self::termion::event::Event::Mouse(m) => {
                match m {
                    self::termion::event::MouseEvent::Press(mb, x, y) => {
                        ui_state.status =
                            format!("MouseEvent::Press => MouseButton {:?} @ ({}, {})", mb, x, y);

                        return InputEvent::ButtonPress {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   x: x as i32,
                                   y: y as i32,
                                   button: 0, // TODO -> enum to ...
                               };
                    }

                    self::termion::event::MouseEvent::Release(x, y) => {
                        ui_state.status = format!("MouseEvent::Release => @ ({}, {})", x, y);

                        return InputEvent::ButtonRelease {
                                   ctrl: false,
                                   alt: false,
                                   shift: false,
                                   x: x as i32,
                                   y: y as i32,
                                   button: 0, // TODO -> enum to ...
                               };
                    }

                    self::termion::event::MouseEvent::Hold(x, y) => {
                        ui_state.status = format!("MouseEvent::Hold => @ ({}, {})", x, y);
                    }
                };
            }

            self::termion::event::Event::Unsupported(e) => {
                ui_state.status = format!("Event::Unsupported {:?}", e);
            }
        }

        break;
    }

    ::core::event::InputEvent::NoInputEvent
}


fn process_input_events(ui_state: &mut UiState, view: &mut View, ev: InputEvent) {

    ui_state.keys.push(ev.clone());

    let mut clear_keys = true;
    match ev {

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('c'),
        } => {
            if ui_state.keys.len() > 1 {

                let prev_ev = &ui_state.keys[ui_state.keys.len() - 2];
                match *prev_ev {
                    InputEvent::KeyPress {
                        ctrl: true,
                        alt: false,
                        shift: false,
                        key: Key::UNICODE('x'),
                    } => {
                        ui_state.quit = true;
                        clear_keys = false;
                    }
                    _ => {}
                }
            } else {
                clear_keys = true;
            }
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('x'),
        } => {
            clear_keys = false;
        }

        // ctrl+?
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE(_),
        } => {}

        // left
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Left,
        } => {
            for m in &mut view.moving_marks {

                // TODO: mark_move_backward(&mut m, &view);

                if m.offset > 0 {
                    m.offset -= 1;
                }
            }
            ui_state.status = format!("<left>");

        }

        // right
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Right,
        } => {
            let doc = view.document.as_mut().unwrap().borrow_mut();
            let buffer_size = doc.buffer.size as u64;

            for m in &mut view.moving_marks {

                // TODO: mark_move_forward(&mut m, &view);

                m.offset += 1;
                if m.offset > buffer_size {
                    m.offset = buffer_size
                }
            }

            ui_state.status = format!("<right>");
        }

        _ => {}
    }

    if clear_keys {
        ui_state.keys.clear();
    }

}

fn display_status_line(ui_state: &UiState,
                       view: &View,
                       line: u16,
                       width: u16,
                       mut stdout: &mut Stdout) {

    let doc = match view.document {
        Some(ref d) => d.borrow(),
        None => return,
    };

    let name = doc.name.as_str();
    let file_name = doc.buffer.file_name.as_str();

    // select/clear last line
    terminal_cursor_to(&mut stdout, 1, line);
    terminal_clear_current_line(&mut stdout, width);
    terminal_cursor_to(&mut stdout, 1, line);

    let status_str = format!("line {} document_name '{}' \
                             , file('{}'), event('{}') \
                             last_offset({}) mark({}) keys({})",
                             line,
                             name,
                             file_name,
                             ui_state.status,
                             ui_state.last_offset,
                             ui_state.mark_offset,
                             ui_state.keys.len());

    print!("{}", status_str);
    stdout.flush().unwrap();
}
