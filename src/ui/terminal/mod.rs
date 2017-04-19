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

    let mut view_state = UiState::new();

    let (width, height) = terminal_size().unwrap();

    setup_views(editor, width as usize, height as usize);

    //
    let stdout = MouseTerminal::from(io::stdout().into_raw_mode().unwrap());
    let mut stdout = AlternateScreen::from(stdout);

    write!(stdout, "{}{}", termion::cursor::Hide, termion::clear::All).unwrap();
    stdout.flush().unwrap();

    while !view_state.quit {

        view_state.nb_view = editor.view_map.len();
        let mut view = editor.view_map.get_mut(&view_state.vid);

        let status_line_y = height;

        if view_state.display_view == true {
            draw_view(&mut view.as_mut().unwrap(), &mut stdout);
        }

        if view_state.display_status == true {
            display_status_line(&mut view.as_mut().unwrap(),
                                &view_state.status,
                                status_line_y,
                                width,
                                &mut stdout);
        }

        process_input_events(&mut view_state);
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


fn fill_screen(view: &mut View) {

    match view.document {

        Some(ref buf) => {

            let data = &buf.borrow().buffer.data;

            view.screen.clear();

            view.start_offset = 0;

            let mut offset = 0;
            for c in data {

                let mut displayed_cp: char = *c as char;
                if *c as char == '\n' as char {
                    displayed_cp = ' ';
                }

                let cpi = CodepointInfo {
                    cp: *c as char,
                    displayed_cp: displayed_cp,
                    offset: offset,
                    is_selected: false,
                };

                let (ok, _) = view.screen.push(cpi);
                offset += 1;
                if ok == false {
                    break;
                }
            }
            view.end_offset = offset;

    // brute force for now
            let mut screen = &mut view.screen;

            for m in &buf.borrow().moving_marks {

    // TODO: screen.find_line_by_offset(m.offset) -> Option<&mut Line>
                if m.offset >= view.start_offset && m.offset <= view.end_offset {
                    for l in 0..screen.height {
                        let line = screen.get_mut_line(l).unwrap();
                        for c in 0..line.used {
                            let mut cpi = line.get_mut_cpi(c).unwrap();
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

    for l in 0..screen.height {

        terminal_cursor_to(&mut stdout, 1, (l + 1) as u16);

        let line = screen.get_line(l).unwrap();

        for c in 0..line.used {

            let cpi = line.get_cpi(c).unwrap();

            if cpi.is_selected == true {
                write!(stdout, "{}", termion::style::Invert).unwrap();
            }

            write!(stdout, "{}", cpi.displayed_cp).unwrap();
            write!(stdout, "{}", termion::style::Reset).unwrap();
        }

        for _ in line.used..line.width {
            write!(stdout, " ").unwrap();
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



fn process_input_events(view_state: &mut UiState) {
    for evt in stdin().events() {

        view_state.keys.push(evt.unwrap());
        let evt = view_state.keys[view_state.keys.len() - 1].clone();

    // Print recieved Events...
        match evt {

            Event::Key(k) => {
                match k {
    // Exit.
                    Key::Ctrl('r') => {
                        view_state.keys.clear();
                    }

                    Key::Ctrl('c') => {
                        if view_state.keys.len() > 1 {
                            if let Event::Key(prev_event) =
                                view_state.keys[view_state.keys.len() - 2] {
                                if let Key::Ctrl(prev_char) = prev_event {
                                    if prev_char == 'x' {
                                        view_state.quit = true;
                                        break;
                                    }
                                }
                            }
                        }
                    }

                    Key::Char(c) => {
                        if c == '\n' {
                            view_state.status = format!("'{}'", "<newline>");
                        } else {
                            view_state.status = format!("'{}'", c);
                        }
                    }
                    Key::Alt(c) => view_state.status = format!("Alt-{}", c),
                    Key::Ctrl(c) => view_state.status = format!("Ctrl-{}", c),

                    Key::F(1) => {
                        if view_state.vid > 0 {
                            view_state.vid -= 1;
                        }
                        break;
                    }

                    Key::F(2) => {
                        view_state.vid = ::std::cmp::min(view_state.vid + 1,
                                                         (view_state.nb_view - 1) as u64);
                        break;
                    }

                    Key::F(f) => view_state.status = format!("F{:?}", f),
                    Key::Left => view_state.status = format!("<left>"),
                    Key::Right => view_state.status = format!("<right>"),
                    Key::Up => view_state.status = format!("<up>"),
                    Key::Down => view_state.status = format!("<down>"),
                    Key::Backspace => view_state.status = format!("<backspace>"),
                    Key::Home => view_state.status = format!("<Home>"),
                    Key::End => view_state.status = format!("<End>"),
                    Key::PageUp => view_state.status = format!("<PageUp>"),
                    Key::PageDown => view_state.status = format!("<PageDown>"),
                    Key::Delete => view_state.status = format!("<Delete>"),
                    Key::Insert => view_state.status = format!("<Insert>"),
                    Key::Esc => view_state.status = format!("<Esc>"),
                    _ => view_state.status = format!("Other"),
                }
            }

            Event::Mouse(m) => {
                match m {
                    MouseEvent::Press(mb, x, y) => {
                        view_state.status =
                            format!("MouseEvent::Press => MouseButton {:?} @ ({}, {})", mb, x, y);
                    }

                    MouseEvent::Release(x, y) => {
                        view_state.status = format!("MouseEvent::Release => @ ({}, {})", x, y);
                    }

                    MouseEvent::Hold(x, y) => {
                        view_state.status = format!("MouseEvent::Hold => @ ({}, {})", x, y);
                    }
                };
            }

            Event::Unsupported(e) => {
                view_state.status = format!("Event::Unsupported {:?}", e);
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
