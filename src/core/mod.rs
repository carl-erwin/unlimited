//
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

pub mod buffer;
pub mod bufferlog;
pub mod codec;
pub mod codepointinfo;
pub mod config;
pub mod document;
pub mod editor;
pub mod event;
pub mod mapped_file;
pub mod mark;
pub mod screen;
pub mod server;
pub mod view;

use core::config::Config;
use core::editor::Editor;
use core::event::Event;

pub const VERSION: &'static str = env!("CARGO_PKG_VERSION");

/// This thread is the "❤" of unlimited.
pub fn start(config: Config, core_rx: Receiver<Event>, ui_tx: Sender<Event>) {
    let mut editor = Editor::new(config);
    // editor.setup_default_buffers(); // scratch , debug
    editor.load_files();
    server::start(&mut editor, core_rx, ui_tx)
}
