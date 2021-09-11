mod terminal;

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use crate::core::event::EventMessage;

pub fn main_loop(
    ui_name: &str,
    ui_rx: &Receiver<EventMessage<'static>>,
    ui_tx: &Sender<EventMessage<'static>>,
    core_tx: &Sender<EventMessage<'static>>,
) {
    // TODO(ceg): switch ui here
    match ui_name {
        "crossterm" | _ => {
            terminal::crossterm::main_loop(&ui_rx, &ui_tx, &core_tx).ok();
        }
    }

    // gtk::main_loop();
    // webui::main_loop();
}

struct UiState {
    quit: bool,
    terminal_width: u16,
    terminal_height: u16,
}

impl UiState {
    fn new() -> Self {
        UiState {
            quit: false,
            terminal_width: 0,
            terminal_height: 0,
        }
    }
}
