// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

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

use crate::core::config::Config;
use crate::core::editor::Editor;
use crate::core::event::EventMessage;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");

/// This thread is the "❤" of unlimited.
pub fn start(config: Config, core_rx: &Receiver<EventMessage>, ui_tx: &Sender<EventMessage>) {
    let mut editor = Editor::new(config);
    // editor.setup_default_buffers(); // scratch , debug
    editor.load_files();
    server::start(&mut editor, &core_rx, &ui_tx)
}
