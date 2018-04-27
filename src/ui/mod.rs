mod terminal;

use std::convert::AsRef;
use std::rc::Rc;
use std::cell::RefCell;

use core::editor::Editor;
use core::event::InputEvent;
use core::view::{build_screen_layout, View};
use core::event::Key;

pub fn main_loop(editor: &mut Editor) {
    // TODO: switch ui here
    let ui = editor.config.ui_frontend.clone();
    match ui.as_ref() {
        "ncurses" => {
            terminal::ncurses::main_loop(editor);
        }

        "termion" | _ => {
            terminal::termion::main_loop(editor);
        }
    }

    // gtk::main_loop();
    // webui::main_loop();
}

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

#[allow(dead_code)] // TODO: even the code is used there is a warning .. we should fill a bug against rustc
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
