use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;

use core::event::Event;

pub fn start(core_rx: Receiver<Event>, ui_tx: Sender<Event>) {
    loop {
        println!("core : waiting for event");
        match core_rx.recv() {
            Ok(evt) => {
                println!("core : recv event : {:?}", evt);
            }
            _ => {
                println!("core : recv error");
            }
        }
        panic!("");
    }
}

pub fn stop() {}
