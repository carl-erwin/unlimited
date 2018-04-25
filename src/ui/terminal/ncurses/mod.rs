extern crate ncurses;
use self::ncurses::*;

use core::editor::Editor;

pub fn main_loop(editor: &mut Editor) {
    /* If your locale env is unicode, you should use `setlocale`. */
    // let locale_conf = LcCategory::all;
    // setlocale(locale_conf, "zh_CN.UTF-8"); // if your locale is like mine(zh_CN.UTF-8).

    /* Start ncurses. */
    initscr();


    loop {
        /* Get the screen bounds. */
        let mut width = 0;
        let mut height = 0;
        getmaxyx(stdscr(), &mut height, &mut width);
        let msg = format!("width = {}, height = {}\n", width, height);

        clear();
        printw("Hello, world!\n");
        printw(&msg);

        /* Update the screen. */
        refresh();

        /* Wait for a key press. */
        let key = getch();
        if key == 'q' as i32 {
            break;
        }
    }
    /* Terminate ncurses. */
    endwin();
}
