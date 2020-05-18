#[macro_export]
#[doc(hidden)]
macro_rules! dbg_println {
    ($($arg:tt)*) => {{

        use std::sync::atomic::Ordering;
        use crate::core::DBG_PRINTLN_FLAG;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            eprintln!($($arg)*)
        }
    }};
}
