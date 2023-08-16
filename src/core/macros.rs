#[macro_export]
#[doc(hidden)]
macro_rules! dbg_println {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            eprint!("[{}] {}:{} ", crate::core::BOOT_TIME.elapsed().unwrap().as_millis(), file!(), line!());
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
            eprint!("[{}] {}:{} ", crate::core::BOOT_TIME.elapsed().unwrap().as_millis(), file!(), line!());
            eprint!($($arg)*)
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! trace_block {
    ($trace_label:expr, $($arg:tt)*) => {

        dbg_println!("{} START", $trace_label);

        let now = std::time::SystemTime::now();

        $($arg)*

        dbg_println!("{} END", $trace_label);


        if crate::core::DBG_PRINTLN_FLAG.load(std::sync::atomic::Ordering::Relaxed) != 0 {
            eprintln!("-- trace_block [{} ms] {} ({}:{}) ", now.elapsed().unwrap().as_millis(), $trace_label, file!(), line!());
        }

    };
}
