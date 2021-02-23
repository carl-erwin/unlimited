// Copyright (c) Carl-Erwin Griffith

pub mod core;
pub mod ui;

// pub mod utils
pub fn sort_tuple_pair<T: PartialOrd>(t: (T, T)) -> (T, T) {
    if t.0 > t.1 {
        (t.1, t.0)
    } else {
        t
    }
}
