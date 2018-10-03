//
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

//
use core::event::Event;

pub fn main_loop(_ui_rx: &Receiver<Event>, core_tx: &Sender<Event>) {
    eprintln!("ncurses frontend is currently disabled (no multithread ui/core support)");

    let ev = Event::ApplicationQuitEvent;
    core_tx.send(ev).unwrap_or(());
}
