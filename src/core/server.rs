use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;

use core::event::Event;

pub fn start(core_rx: Receiver<Event>, ui_tx: Sender<Event>) {
    loop {
        println!("core : waiting for event");
        match core_rx.recv() {
            Ok(evt) => {
                println!("core : recv event : {:?}", evt);

                match evt {
                    ApplicationQuitEvent => {
                        let ev = Event::ApplicationQuitEvent;
                        ui_tx.send(ev);

                        break;
                    }
                    _ => {}
                }
            }
            _ => {
                println!("core : recv error");
            }
        }
    }
}

pub fn stop() {}
