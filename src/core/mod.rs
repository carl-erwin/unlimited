//
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;

pub mod editor;
pub mod config;
pub mod screen;
pub mod codepointinfo;
pub mod document;
pub mod buffer;
pub mod bufferlog;
pub mod mapped_file;
pub mod event;
pub mod view;
pub mod mark;
pub mod codec;
pub mod server;

use core::event::Event;

/// not implemented : This function starts the core thread.<br/>
/// This thread will be the "❤" of unlimited.
pub fn start(core_rx: Receiver<Event>, ui_tx: Sender<Event>) {
    server::start(core_rx, ui_tx)
}

/// not implemented : This function stops the core thread.
// not implemented : TODO: return a status , ex waiting for job to finsh etc
pub fn stop() {
    server::stop()
}
