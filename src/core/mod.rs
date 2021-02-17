// Copyright (c) Carl-Erwin Griffith

//
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;
use std::thread;

#[macro_use]
pub(crate) mod macros;

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
pub mod view;

use crate::core::config::Config;
use crate::core::editor::Editor;
use crate::core::editor::EditorEnv;
use crate::core::event::EventMessage;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// This thread is the "❤" of unlimited.
pub fn run(config: Config, core_rx: &Receiver<EventMessage>, ui_tx: &Sender<EventMessage>) {
    let mut editor = Editor::new(config);
    let mut env = EditorEnv::new();

    editor.load_files(&mut env);

    // TODO: every sent msg must have
    // reply_to: &Sender<EventMessage>
    // start indexer thread
    let indexer_th = {
        // let ui_tx_clone = ui_tx.clone();
        Some(thread::spawn(move || {
            dbg_println!("stating indexer");
        }))
    };

    editor::run(&mut editor, &mut env, &core_rx, &ui_tx);

    // wait for core indexer thread
    if let Some(th) = indexer_th {
        th.join().unwrap()
    }
}
