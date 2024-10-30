#[macro_export]
#[doc(hidden)]
macro_rules! dbg_println {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;

        use std::io::Write;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            let mut f = crate::core::get_log_file().lock().expect("failed to lock log file");
            writeln!(f, "[{}] {}:{} ", crate::core::BOOT_TIME.elapsed().unwrap().as_millis(), file!(), line!()).unwrap();
            writeln!(f, $($arg)*).unwrap();
        }
    }};
}

#[macro_export]
#[doc(hidden)]
macro_rules! dbg_print {
    ($($arg:tt)*) => {{
        use crate::core::DBG_PRINTLN_FLAG;
        use std::sync::atomic::Ordering;
        use std::io::Write;

        if DBG_PRINTLN_FLAG.load(Ordering::Relaxed) != 0 {
            let mut f = crate::core::get_log_file().lock().expect("failed to lock log file");
            write!(f, "[{}] {}:{} ", crate::core::BOOT_TIME.elapsed().unwrap().as_millis(), file!(), line!()).unwrap();
            write!(f, $($arg)*).unwrap();
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
