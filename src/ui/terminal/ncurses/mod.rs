//
use std::thread;
use std::rc::Rc;
use std::cell::RefCell;

extern crate ncurses;
use self::ncurses::*;

//
use core::view::{build_screen_layout, View};
use core::screen::Screen;
use core::event::InputEvent;
use core::event::Key;
use core::editor::Editor;

// move to ui/mods.rs

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
    input_wait_time_ms: u64,
    terminal_width: u16,
    terminal_height: u16,
    view_start_line: usize,
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
            input_wait_time_ms: 20,
            terminal_width: 0,
            terminal_height: 0,
            view_start_line: 0,
        }
    }
}

pub fn main_loop(mut editor: &mut Editor) {
    let mut ui_state = UiState::new();

    /* If your locale env is unicode, you should use `setlocale`. */
    // let locale_conf = LcCategory::all;
    // setlocale(locale_conf, "zh_CN.UTF-8"); // if your locale is like mine(zh_CN.UTF-8).

    /* Start ncurses. */
    initscr();
    curs_set(CURSOR_VISIBILITY::CURSOR_INVISIBLE);
    keypad(stdscr(), true); /* for F1, arrow etc ... */
    clear();
    noecho();
    raw();
    halfdelay(1); // 20ms

    /* Get the screen bounds. */
    let mut width = 0;
    let mut height = 0;
    getmaxyx(stdscr(), &mut height, &mut width);

    ui_state.terminal_width = width as u16;
    ui_state.terminal_height = height as u16;
    ui_state.view_start_line = 0;

    setup_views(&mut editor, width as usize, height as usize);

    while !ui_state.quit {
        ui_state.nb_view = editor.view_map.len();
        let mut view = Some(&editor.view_map[ui_state.vid as usize].1);
        getmaxyx(stdscr(), &mut height, &mut width);

        // check screen size
        {
            let mut view = view.as_mut().unwrap().borrow_mut();
            if width != ui_state.terminal_width as i32 || height != ui_state.terminal_height as i32
            {
                view.screen = Box::new(Screen::new(
                    ui_state.terminal_width as usize,
                    ui_state.terminal_height as usize,
                ));
            }
        }

        if ui_state.display_view {
            draw_view(&mut ui_state, &mut view.as_mut().unwrap().borrow_mut());
        }

        let vec_evt = get_input_event(&mut ui_state);
        for evt in vec_evt {
            process_input_events(
                &mut ui_state,
                &mut view.as_mut().unwrap().borrow_mut(),
                &evt,
            );

            // re-sync view on each event/update or else the main mark will be offscreen
            // TODO: add a view flag to call this
            fill_screen(&mut ui_state, &mut view.as_mut().unwrap().borrow_mut());
        }
    }
    /* Terminate ncurses. */
    endwin();
}

fn setup_views(editor: &mut Editor, width: usize, height: usize) {
    let mut views = Vec::new();

    for (vid, b) in editor.document_map.iter().enumerate() {
        let view = View::new(
            vid as u64,
            0 as u64,
            width as usize,
            height as usize,
            Some(b.1.clone()),
        );
        views.push(view);
    }

    for view in views {
        editor.view_map.push((view.id, Rc::new(RefCell::new(view))));
    }
}

/*
    TODO:
    1 : be explicit
    2 : create editor internal result type Result<>
    3 : use idomatic    func()? style
*/
fn draw_view(mut ui_state: &mut UiState, mut view: &mut View) {
    let start_line = ui_state.view_start_line;
    fill_screen(&mut ui_state, &mut view);
    draw_screen(&mut view.screen, start_line);
}

/// move to ui/layout
fn fill_screen(ui_state: &mut UiState, view: &mut View) {
    if let Some(ref _buf) = view.document {
        let mut screen = &mut view.screen;

        screen.clear();

        let mut data = vec![];
        let max_size = (screen.width * screen.height * 4) as usize;
        let doc = view.document.as_ref().unwrap().borrow_mut();
        doc.read(view.start_offset, max_size, &mut data);

        let max_offset = doc.buffer.size as u64;

        view.end_offset = build_screen_layout(&data, view.start_offset, max_offset, &mut screen);

        ui_state.last_offset = view.end_offset;

        // render marks

        // brute force for now
        for m in view.moving_marks.borrow().iter() {
            // TODO: screen.find_line_by_offset(m.offset) -> Option<&mut Line>
            if m.offset >= view.start_offset && m.offset <= view.end_offset {
                for l in 0..screen.height {
                    let line = screen.get_mut_line(l).unwrap();
                    for c in 0..line.nb_cells {
                        let cpi = line.get_mut_cpi(c).unwrap();

                        //if cpi.offset > m.offset {
                        //break;
                        //}

                        if cpi.offset == m.offset {
                            cpi.is_selected = true;
                            ui_state.mark_offset = m.offset;
                        }
                    }
                }
            }
        }
    }
}

fn draw_screen(screen: &mut Screen, start_line: usize) {
    clear();

    for li in 0..screen.height {
        mv(1, li as i32);

        let line = screen.get_line(li).unwrap();

        for c in 0..line.width {
            let cpi = line.get_cpi(c).unwrap();

            if cpi.is_selected {
                attron(A_REVERSE());
            }
            if cpi.cp < 128 as char {
                mvaddch(li as i32, c as i32, cpi.displayed_cp as u64);
            } else {
                //                u8 buff[8];
                //                utf8_put_cp(cpi->codepoint, buff);
                //                mvaddstr(li, c, (const char *)buff);
            }

            if cpi.is_selected {
                attroff(A_REVERSE());
            }
        }

        /*
        for _ in line.nb_cells..line.width {
            write!(stdout, " ").unwrap();
        }
        */
    }

    /* Update the screen. */
    refresh();
}

fn get_input_event(ui_state: &mut UiState) -> Vec<InputEvent> {
    let mut v = Vec::<InputEvent>::new();

    let keycode = wgetch(stdscr()); /* Wait for user input */
    // FIXE: use timer to check core event
    if keycode == ERR {
        return v;
    }

    ui_state.status = format!(" keycode = {}", keycode);

    let name = keyname(keycode);

    match keycode {
        KEY_UP => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Up,
            });
        }

        KEY_DOWN => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Down,
            });
        }

        KEY_LEFT => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Left,
            });
        }
        KEY_RIGHT => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::Right,
            });
        }

        _ => {
            /* TODO */

            if keycode == 'q' as i32 {
                ui_state.quit = true;
            }
        }
    } // ! switch(keycode)

    v
}

fn process_input_events(ui_state: &mut UiState, view: &mut View, ev: &InputEvent) {
    if *ev == ::core::event::InputEvent::NoInputEvent {
        // ignore no input event event :-)
        return;
    }

    ui_state.keys.push(ev.clone());

    let mut clear_keys = true;
    match *ev {
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('q'),
        } => {
            ui_state.quit = true;
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('u'),
        } => {
            view.undo();
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('r'),
        } => {
            view.redo();
        }

        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('x'),
        } => {
            clear_keys = false;
        }

        // ctrl+a
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('a'),
        }
        | InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Home,
        } => {
            view.move_marks_to_beginning_of_line();
        }

        // ctrl+e
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('e'),
        }
        | InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::End,
        } => {
            view.move_marks_to_end_of_line();
        }

        // ctrl+d
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('d'),
        } => {
            view.remove_codepoint();
        }

        // ctrl+s
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('s'),
        } => {
            view.save_document();
        }

        // ctrl+k
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('k'),
        } => {
            view.cut_to_end_of_line();
        }

        // ctrl+y
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('y'),
        } => {
            view.paste();
        }

        // ctrl+l
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE('l'),
        } => {
            view.center_arround_mark();
        }

        // ctrl+?
        InputEvent::KeyPress {
            ctrl: true,
            alt: false,
            shift: false,
            key: Key::UNICODE(c),
        } => {
            ui_state.status = format!("ctrl+<{}>", c);
        }

        // left
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Left,
        } => {
            view.move_marks_backward();

            ui_state.status = "<left>".to_owned();
        }

        // up
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Up,
        } => {
            view.move_marks_to_previous_line();

            ui_state.status = "<up>".to_owned();
        }

        // down
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Down,
        } => {
            view.move_marks_to_next_line();

            ui_state.status = "<down>".to_owned();
        }

        // right
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Right,
        } => {
            view.move_marks_forward();

            ui_state.status = "<right>".to_owned();
        }

        // page_up
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::PageUp,
        } => {
            view.scroll_to_previous_screen();
            ui_state.status = "<page_up>".to_owned();
        }

        // page_down
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::PageDown,
        } => {
            view.scroll_to_next_screen();
            ui_state.status = "<page_down>".to_owned();
        }

        // alt+< goto beginning of file
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::UNICODE('<'),
        } => {
            view.move_mark_to_beginning_of_file();
            ui_state.status = "<move to beginning of file>".to_owned();
        }

        // alt+> goto end of file
        InputEvent::KeyPress {
            ctrl: false,
            alt: true,
            shift: false,
            key: Key::UNICODE('>'),
        } => {
            view.move_mark_to_end_of_file();
            ui_state.status = "<move to end of file>".to_owned();
        }

        // delete
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::Delete,
        } => {
            view.remove_codepoint();
            ui_state.status = "<del>".to_owned();
        }

        // backspace
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::BackSpace,
        } => {
            view.remove_previous_codepoint();

            ui_state.status = "<backspace>".to_owned();
        }

        // insert text
        InputEvent::KeyPress {
            ctrl: false,
            alt: false,
            shift: false,
            key: Key::UNICODE(cp),
        } => {
            view.insert_codepoint(cp);
            ui_state.status = format!("<insert [0x{:x}]>", cp as u32);
        }

        // mouse button pressed
        InputEvent::ButtonPress {
            ctrl: false,
            alt: false,
            shift: false,
            x,
            y,
            button,
        } => match button {
            0 | 1 => {
                view.button_press(button, x, y - 2);
                ui_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }
            3 => {
                view.scroll_up(3);
                ui_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }
            4 => {
                view.scroll_down(3);
                ui_state.status = format!("<click({},@({},{}))]>", button, x, y - 2);
            }

            _ => {}
        },

        // mouse button released
        InputEvent::ButtonRelease {
            ctrl: false,
            alt: false,
            shift: false,
            x,
            y,
            button,
        } => {
            view.button_release(button, x, y);
            ui_state.status = format!("<unclick({},@({},{}))]>", button, x, y);
        }

        _ => {}
    }

    if clear_keys {
        ui_state.keys.clear();
    }
}
