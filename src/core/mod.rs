//
pub mod editor;
pub mod config;
pub mod screen;
pub mod codepointinfo;
pub mod document;
pub mod buffer;
pub mod event;
pub mod view;
pub mod mark;

pub mod codec;

pub mod server;


use std::io::{stderr, Stderr, Write};



// TODO: move to better module : core::utils ?
pub struct OffsetRange {
    pub start: u64,
    pub end: u64,
}

// start core thread
pub fn start() {
    server::start()
}


// TODO: return a status , ex waiting for job to finsh etc
pub fn stop() {
    server::stop()
}
