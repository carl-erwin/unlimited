extern crate termion;

pub mod editor;
pub mod config;
pub mod screen;
pub mod codepointinfo;
pub mod buffer;
pub mod byte_buffer;
pub mod event;
pub mod view;
pub mod mark;


// start main thread
pub fn start() {}


// TODO: return a status , ex waiting for job to finsh etc
pub fn stop() {}
