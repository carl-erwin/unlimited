//
extern crate termion;


//
use self::termion::screen::AlternateScreen;
use self::termion::event::{Event, Key, MouseEvent};
use self::termion::input::{TermRead, MouseTerminal};
use self::termion::raw::IntoRawMode;
use self::termion::terminal_size;

use std::io::{self, Write, stdin, Stdout};

//
use core::view::View;
use core::screen::Screen;
use core::codepointinfo::CodepointInfo;

use core::editor::Editor;

use core::text_codec::utf8_decode_byte;
use core::text_codec::{UTF8_ACCEPT, UTF8_REJECT};

use core::text_codec::u32_to_char;


//
struct UiState {
    keys: Vec<Event>,
    quit: bool,
    status: String,
    display_status: bool,
    display_view: bool,
    vid: u64,
    nb_view: usize,
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
            draw_view(&mut view.as_mut().unwrap(), &mut stdout);
        }

        if ui_state.display_status == true {
            display_status_line(&mut view.as_mut().unwrap(),
                                &ui_state.status,
                                status_line_y,
                                width,
                                &mut stdout);
        }

        process_input_events(&mut ui_state, &mut view.as_mut().unwrap());
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

fn screen_putchar(mut screen: &mut Screen, c: char, offset: u64) -> bool {

    let mut displayed_cp = c;
    if c == '\n' as char {
        displayed_cp = ' ';
    }

    let cpi = CodepointInfo {
        cp: c,
        displayed_cp: displayed_cp,
        offset: offset,
        is_selected: false,
    };

    let (ok, _) = screen.push(cpi);
    ok
}


fn decode_slice(data: &[u8],
                base_offset: u64,
                mut screen: &mut Screen,
                cb: fn(&mut Screen, char, u64) -> bool)
                -> u64 {

    let debug_error = false;
    let mut offset = base_offset;
    let mut state: u32 = 0;
    let mut cp_val: u32 = 0;
    let mut cp_start_offset = base_offset;

    for b in data {
        let cp: char;

        state = utf8_decode_byte(&mut state, *b, &mut cp_val);

        if debug_error == true {
            let s = &format!(" |decoding byte {:x} sequence @ offset {}\n", *b, offset);
            screen_putstr(&mut screen, &s);
        }

        match state {
            UTF8_ACCEPT => {
                cp = u32_to_char(cp_val);
                cb(&mut screen, cp, cp_start_offset);
                cp_start_offset = offset + 1;

                // reset state
                cp_val = 0;
                state = UTF8_ACCEPT;
            }

            UTF8_REJECT => {
                if debug_error == true {
                    let s = &format!(" |error decoding byte {:x} sequence @ offset {}\n",
                                     *b,
                                     offset);
                    screen_putstr(&mut screen, &s);
                }

                for i in cp_start_offset..offset + 1 {
                    cb(&mut screen, '�', i);
                }
                cp_start_offset = offset + 1;

                // reset state
                cp_val = 0;
                state = UTF8_ACCEPT;
            }

            _ => { /* intermediate state */ }
        }

        offset += 1;
    }

    offset
}


fn fill_screen(mut view: &mut View) {

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

            // TODO: return -> Vec<CodepointInfo>
            // let max_cp = ::std::cmp::min(data.len(), screen.width * screen.height * 4);
            view.end_offset = decode_slice(&data[0..len],
                                           view.start_offset,
                                           &mut screen,
                                           screen_putchar);

            if view.end_offset == buf.borrow().buffer.size as u64 {
                screen.push(CodepointInfo {
                                cp: ' ',
                                displayed_cp: ' ',
                                offset: view.end_offset,
                                is_selected: false,
                            });
            }

            // brute force for now
            for m in &buf.borrow().moving_marks {

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
fn draw_view(mut view: &mut View, mut stdout: &mut Stdout) {

    fill_screen(&mut view);
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



fn process_input_events(ui_state: &mut UiState, view: &mut View) {
    for evt in stdin().events() {

        ui_state.keys.push(evt.unwrap());
        let evt = ui_state.keys[ui_state.keys.len() - 1].clone();

        // Print recieved Events...
        match evt {

            Event::Key(k) => {
                match k {
                    // Exit.
                    Key::Ctrl('r') => {
                        ui_state.keys.clear();
                    }

                    Key::Ctrl('c') => {
                        if ui_state.keys.len() > 1 {
                            if let Event::Key(prev_event) = ui_state.keys[ui_state.keys.len() - 2] {
                                if let Key::Ctrl(prev_char) = prev_event {
                                    if prev_char == 'x' {
                                        ui_state.quit = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    Key::Char(c) => {
                        if c == '\n' {
                            ui_state.status = format!("'{}'", "<newline>");
                        } else {
                            ui_state.status = format!("'{}'", c);
                        }
                    }
                    Key::Alt(c) => ui_state.status = format!("Alt-{}", c),
                    Key::Ctrl(c) => ui_state.status = format!("Ctrl-{}", c),

                    Key::F(1) => {
                        if ui_state.vid > 0 {
                            ui_state.vid -= 1;
                        }
                        break;
                    }

                    Key::F(2) => {
                        ui_state.vid = ::std::cmp::min(ui_state.vid + 1,
                                                       (ui_state.nb_view - 1) as u64);
                        break;
                    }

                    Key::F(f) => ui_state.status = format!("F{:?}", f),
                    Key::Left => {
                        ui_state.status = {
                            let mut doc = view.document.as_mut().unwrap().borrow_mut();
                            for m in &mut doc.moving_marks {
                                if m.offset > 0 {
                                    m.offset -= 1;
                                }
                            }
                            format!("<left>")
                        }
                    }
                    Key::Right => {
                        ui_state.status = {
                            let mut doc = view.document.as_mut().unwrap().borrow_mut();
                            let buffer_size = doc.buffer.size as u64;

                            for m in &mut doc.moving_marks {
                                m.offset += 1;
                                if m.offset > buffer_size {
                                    m.offset = buffer_size
                                }
                            }

                            format!("<right>")
                        }
                    }
                    Key::Up => ui_state.status = format!("<up>"),
                    Key::Down => ui_state.status = format!("<down>"),
                    Key::Backspace => ui_state.status = format!("<backspace>"),
                    Key::Home => ui_state.status = format!("<Home>"),
                    Key::End => ui_state.status = format!("<End>"),
                    Key::PageUp => ui_state.status = format!("<PageUp>"),
                    Key::PageDown => ui_state.status = format!("<PageDown>"),
                    Key::Delete => ui_state.status = format!("<Delete>"),
                    Key::Insert => ui_state.status = format!("<Insert>"),
                    Key::Esc => ui_state.status = format!("<Esc>"),
                    _ => ui_state.status = format!("Other"),
                }
            }

            Event::Mouse(m) => {
                match m {
                    MouseEvent::Press(mb, x, y) => {
                        ui_state.status =
                            format!("MouseEvent::Press => MouseButton {:?} @ ({}, {})", mb, x, y);
                    }

                    MouseEvent::Release(x, y) => {
                        ui_state.status = format!("MouseEvent::Release => @ ({}, {})", x, y);
                    }

                    MouseEvent::Hold(x, y) => {
                        ui_state.status = format!("MouseEvent::Hold => @ ({}, {})", x, y);
                    }
                };
            }

            Event::Unsupported(e) => {
                ui_state.status = format!("Event::Unsupported {:?}", e);
            }
        }

        break;
    }
}

fn display_status_line(view: &View, status: &str, line: u16, width: u16, mut stdout: &mut Stdout) {

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

    let status_str = format!("line {} document_name '{}', file: '{}', event '{}'",
                             line,
                             name,
                             file_name,
                             status);

    print!("{}", status_str);
    stdout.flush().unwrap();
}
