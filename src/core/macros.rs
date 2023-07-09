#[macro_export]
#[doc(hidden)]
macro_rules! dbg_println {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            eprint!("[{}] {}:{} ", crate::core::BOOT_TIME.elapsed().unwrap().as_millis() ,file!(), line!());
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
            eprint!("[{}] {}:{} ", crate::core::BOOT_TIME.elapsed().unwrap().as_millis() ,file!(), line!());
            eprint!($($arg)*)
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! trace_block {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;

        let now = std::time::SystemTime::now();

        $($arg)*

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            eprintln!("trace_block [{}] {}:{} ", now.elapsed().unwrap().as_millis(),file!(), line!());
        }

    }};
}
