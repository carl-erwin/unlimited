//
extern crate ncurses;

use self::ncurses::*;

//
use core::view::View;
use core::screen::Screen;
use core::event::InputEvent;
use core::event::Key;
use core::editor::Editor;

use ui::UiState;
use ui::setup_views;
use ui::fill_screen;
use ui::process_input_events;

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

    if ui_state.input_wait_time_ms != 0 {
        halfdelay(1); // 20ms
    }

    if ui_state.display_status {
        // TODO
    }

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

fn draw_screen(screen: &mut Screen, _start_line: usize) {
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

    let _name = keyname(keycode);

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

        KEY_PPAGE => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::PageUp,
            });
        }

        KEY_NPAGE => {
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::PageDown,
            });
        }

        0...26 => {
            let mut ctrl = false;
            let mut c;

            let k = unsafe { ::std::char::from_u32_unchecked(keycode as u32 + ('a' as u32) - 1) };
            match k {
                'j' => {
                    c = '\n';
                }

                'i' => {
                    c = '\t';
                }

                _ => {
                    c = unsafe { ::std::char::from_u32_unchecked(k as u32) };
                    ctrl = true;
                }
            }

            v.push(InputEvent::KeyPress {
                ctrl: ctrl,
                alt: false,
                shift: false,
                key: Key::UNICODE(c),
            });
        }

        27...127 => {
            /* TODO */
            let k = unsafe { ::std::char::from_u32_unchecked(keycode as u32) };
            v.push(InputEvent::KeyPress {
                ctrl: false,
                alt: false,
                shift: false,
                key: Key::UNICODE(k),
            });
        }

        _ => {}
    } // ! switch(keycode)

    v
}
