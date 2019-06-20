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

mod terminal;

use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use crate::core::event::EventMessage;

pub fn main_loop(
    ui_name: &str,
    ui_rx: &Receiver<EventMessage>,
    ui_tx: &Sender<EventMessage>,
    core_tx: &Sender<EventMessage>,
) {
    // TODO: switch ui here
    match ui_name {
        "termion" | _ => {
            terminal::termion::main_loop(&ui_rx, &ui_tx, &core_tx);
        }
    }

    // gtk::main_loop();
    // webui::main_loop();
}

struct UiState {
    quit: bool,
    terminal_width: u16,
    terminal_height: u16,
    resize_flag: bool,
}

impl UiState {
    fn new() -> Self {
        UiState {
            quit: false,
            terminal_width: 0,
            terminal_height: 0,
            resize_flag: false,
        }
    }
}
