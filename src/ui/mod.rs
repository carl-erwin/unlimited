mod terminal;

use std::convert::AsRef;
use core::editor::Editor;

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
