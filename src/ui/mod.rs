mod graphical;
mod terminal;

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use crate::core::config::ConfigVariables;
use crate::core::event::Message;

pub fn main_loop(
    config_vars: &ConfigVariables,
    ui_name: &str,
    ui_rx: &Receiver<Message<'static>>,
    ui_tx: &Sender<Message<'static>>,
    core_tx: &Sender<Message<'static>>,
) {
    // TODO(ceg): switch ui here
    match ui_name {
        "crossterm" => {
            terminal::crossterm::main_loop(config_vars, ui_rx, ui_tx, core_tx).ok();
        }

        #[cfg(feature = "gfx-sdl")]
        "sdl" | "sdl2" => {
            graphical::sdl2::main_loop_sdl(ui_rx, ui_tx, core_tx).ok();
        }

        #[cfg(feature = "gfx-sdl")]
        "gl" => {
            graphical::sdl2::main_loop_sdl_gl(ui_rx, ui_tx, core_tx).ok();
        }

        _ => {
            terminal::crossterm::main_loop(config_vars, ui_rx, ui_tx, core_tx).ok();
        }
    }

    // gtk::main_loop();
    // webui::main_loop();
}
