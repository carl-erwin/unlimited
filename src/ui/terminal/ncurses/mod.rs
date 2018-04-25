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

        if ui_state.display_view {
            draw_view(&mut ui_state, &mut view.as_mut().unwrap().borrow_mut());
        }

        /* Wait for a key press. */
        let key = getch();
        if key == 'q' as i32 {
            ui_state.quit = true;
        }

        //        fill_screen(&mut ui_state, &mut view.as_mut().unwrap().borrow_mut());
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
