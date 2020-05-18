/// This module contains the server core
pub mod core;

/// This module contains the ui front-ends.<br/>
/// The current design uses Sender/Receiver to exchange data/state to the core
pub mod ui;

// TODO: pub mod misc
/// simple function to sort a (T, T) pair
pub fn sort_pair<T: PartialOrd>(t: (T, T)) -> (T, T) {
    if t.0 > t.1 {
        (t.1, t.0)
    } else {
        t
    }
}
