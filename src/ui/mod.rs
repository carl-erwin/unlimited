mod terminal;

use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;

use core::event::Event;

pub fn main_loop(ui_name: &str, ui_rx: Receiver<Event>, core_tx: Sender<Event>) {
    // TODO: switch ui here
    match ui_name {
        "ncurses" => {
            // terminal::ncurses::main_loop(ui_rx, core_tx);
        }

        "termion" | _ => {
            terminal::termion::main_loop(ui_rx, core_tx);
        }
    }

    // gtk::main_loop();
    // webui::main_loop();
}

struct UiState {
    quit: bool,
    status: String,
    display_status: bool,
    display_view: bool,
    vid: u64,
    nb_view: usize,
    mark_offset: u64,
    input_wait_time_ms: u64,
    terminal_width: u16,
    terminal_height: u16,
    view_start_line: usize,
    resize_flag: bool,
}

impl UiState {
    fn new() -> UiState {
        UiState {
            quit: false,
            status: String::new(),
            display_status: true,
            display_view: true,
            vid: 0,
            nb_view: 0,
            mark_offset: 0,
            input_wait_time_ms: 20,
            terminal_width: 0,
            terminal_height: 0,
            view_start_line: 0,
            resize_flag: false,
        }
    }
}
