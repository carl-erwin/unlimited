// Copyright (c) Carl-Erwin Griffith

//
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

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
pub mod modes;
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
    editor.load_modes(&mut env);
    editor.main_loop(&mut env, &core_rx, &ui_tx);
}
