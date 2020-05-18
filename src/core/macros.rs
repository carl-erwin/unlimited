#[macro_export]
#[doc(hidden)]
macro_rules! dbg_println {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            eprintln!($($arg)*)
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! dbg_print {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            eprint!($($arg)*)
        }
    }};
}
