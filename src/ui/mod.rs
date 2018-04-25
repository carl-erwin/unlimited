mod terminal;

use core::editor::Editor;

pub fn main_loop(editor: &mut Editor) {
    // TODO: switch ui here
    terminal::termion::main_loop(editor);
    // gtk::main_loop();
    // webui::main_loop();
}
