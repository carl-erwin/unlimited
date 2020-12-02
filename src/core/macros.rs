// Copyright (c) Carl-Erwin Griffith

#[macro_export]
#[doc(hidden)]

macro_rules! dbg_println {
    ($($arg:tt)*) => ({
        let key = "UNLIMITED_DEBUG";
        match std::env::var(key) {
            Ok(_) => eprintln!($($arg)*),
            Err(_) => {},
        }
    })
}
