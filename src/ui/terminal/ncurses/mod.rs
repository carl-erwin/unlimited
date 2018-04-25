extern crate ncurses;
use self::ncurses::*;

use core::editor::Editor;

pub fn main_loop(editor: &mut Editor) {
    /* If your locale env is unicode, you should use `setlocale`. */
    // let locale_conf = LcCategory::all;
    // setlocale(locale_conf, "zh_CN.UTF-8"); // if your locale is like mine(zh_CN.UTF-8).

    /* Start ncurses. */
    initscr();

    /* Print to the back buffer. */
    printw("Hello, world!");

    /* Print some unicode(Chinese) string. */
    // printw("Great Firewall dislike VPN protocol.\nGFW 不喜欢 VPN 协议。");

    /* Update the screen. */
    refresh();

    /* Wait for a key press. */
    getch();

    /* Terminate ncurses. */
    endwin();
}
