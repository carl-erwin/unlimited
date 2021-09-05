// std
use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use std::thread;

// ext
extern crate clap;
extern crate unlimited;
#[macro_use]
extern crate lazy_static;

use clap::{App, Arg};

// crate
use unlimited::core;
use unlimited::core::config::Config;
use unlimited::core::VERSION;
use unlimited::ui;

use unlimited::core::event::EventMessage;

/// Program entry point
/// It parses the command line to build the configuration.
/// Creates the core thread.
/// Nb: the created ui runs the main thread
fn main() {
    //
    //check_env_flags();

    let config = parse_command_line();

    // build core/ui communication channels
    let ui_name = config.ui_frontend.clone();
    let (ui_tx, ui_rx) = channel();
    let (core_tx, core_rx) = channel();
    // create core thread
    let core_th = start_core_thread(config, core_tx.clone(), core_rx, ui_tx.clone());

    // run the ui loop in the main thread
    ui::main_loop(ui_name.as_ref(), &ui_rx, &ui_tx, &core_tx);

    // wait for core thread
    if let Some(core_handle) = core_th {
        core_handle.join().unwrap()
    }
}

fn start_core_thread(
    config: Config,
    core_tx: Sender<EventMessage<'static>>,
    core_rx: Receiver<EventMessage<'static>>,
    ui_tx: Sender<EventMessage<'static>>,
) -> Option<thread::JoinHandle<()>> {
    let _tx_ = ui_tx.clone();
    let core_tx = core_tx.clone();

    Some(thread::spawn(move || {
        core::run(config, &core_rx, &core_tx, &ui_tx)
    }))
}

fn _check_env_flags() {
    match std::env::var("UNLIMITED_CHECK_SCREEN_INVARIANTS") {
        Err(_) => return,
        _ => crate::core::screen::enable_screen_checks(),
    }
}

/// Parse command and an return a Config
fn parse_command_line() -> Config {
    let matches = App::new("unlimited")
        .version(VERSION)
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("unlimited is an experimental editor")
        .args_from_usage("--ui, --ui=[termion|crossterm] 'select user interface frontend'")
        .arg(
            Arg::with_name("DEBUG")
                .short("d")
                .long("--debug")
                .help("enable debug logs on stderr (use redirection to file)"), // TODO(ceg): isatty ?
        )
        .arg(
            Arg::with_name("NO_READ_CACHE")
                .long("--no-read-cache")
                .help("disable read cache (debug purpose)"),
        )
        .arg(
            Arg::with_name("NO_BYTE_INDEX")
                .short("n")
                .long("--no-byte-index")
                .help("disable byte index (wip)"),
        )
        .arg(
            Arg::with_name("BENCH_TO_EOF")
                .short("b")
                .long("--bench-to-eof")
                .help("render all screen until EOF is reached and quit (wip: no proper quit yet)"),
        )
        .arg(Arg::with_name("NO_UI_RENDER").long("--no-ui-render"))
        .arg(
            Arg::with_name("RAW_FILTER_TO_SCREEN")
                .short("r") // TODO remove
                .long("--raw-data-to-screen")
                .help("render all screen until EOF is reached and quit (wip: no proper quit yet)"),
        )
        .arg(
            Arg::with_name("FILES")
                .help("list of the files to open")
                .required(false)
                .multiple(true),
        )
        .get_matches();

    let mut ui_frontend = String::new();
    if matches.is_present("ui") {
        ui_frontend = matches.values_of("ui").unwrap().collect::<String>();
    }

    let mut files_list = Vec::new();
    if matches.is_present("FILES") {
        files_list = matches
            .values_of("FILES")
            .unwrap()
            .map(|x| x.to_owned())
            .collect();
    }

    if matches.is_present("DEBUG") {
        core::enable_dbg_println();
        core::screen::enable_screen_checks();
    }

    if matches.is_present("NO_READ_CACHE") {
        core::disable_read_cache();
    }

    if matches.is_present("NO_BYTE_INDEX") {
        core::disable_byte_index();
    }

    if matches.is_present("BENCH_TO_EOF") {
        core::enable_bench_to_eof();
    }

    if matches.is_present("RAW_FILTER_TO_SCREEN") {
        core::enable_raw_data_filter_to_screen();
    }

    if matches.is_present("NO_UI_RENDER") {
        core::set_no_ui_render(true);
    }

    Config {
        files_list,
        ui_frontend,
    }
}
