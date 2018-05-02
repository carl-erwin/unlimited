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

use core::config::Config;
use core::editor::Editor;
use core::event::Event;

/// not implemented : This function starts the core thread.<br/>
/// This thread will be the "❤" of unlimited.
pub fn start(config: Config, core_rx: Receiver<Event>, ui_tx: Sender<Event>) {
    let mut editor = Editor::new(config);
    // editor.setup_default_buffers(); // scratch , debug
    editor.load_files();
    server::start(&mut editor, core_rx, ui_tx)
}
