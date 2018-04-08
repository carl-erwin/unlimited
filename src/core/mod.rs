//
pub mod editor;
pub mod config;
pub mod screen;
pub mod codepointinfo;
pub mod document;
pub mod buffer;
pub mod bufferlog;
pub mod mapped_file;
pub mod event;
pub mod view;
pub mod mark;
pub mod codec;
pub mod server;

/// not implemented : This function starts the core thread.<br/>
/// This thread will be the "❤" of unlimited.
pub fn start() {
    server::start()
}

/// not implemented : This function stops the core thread.
// not implemented : TODO: return a status , ex waiting for job to finsh etc
pub fn stop() {
    server::stop()
}
