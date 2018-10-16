//
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

//
use core::event::Event;
use core::event::EventMessage;

pub fn main_loop(_ui_rx: &Receiver<EventMessage>, core_tx: &Sender<EventMessage>) {
    eprintln!("ncurses frontend is currently disabled (no multithread ui/core support)");

    let msg = EventMessage::new(0, Event::ApplicationQuitEvent);
    core_tx.send(msg).unwrap_or(());
}
