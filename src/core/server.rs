use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;

use core::event::Event;

pub fn start(core_rx: Receiver<Event>, ui_tx: Sender<Event>) {}

pub fn stop() {}
