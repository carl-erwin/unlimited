// std
use std::collections::HashMap;

use std::sync::mpsc::channel;
use std::sync::mpsc::Receiver;
use std::sync::mpsc::Sender;

use std::thread;
//use std::time::Duration;

// ext
#[macro_use]
extern crate clap;
extern crate unlimited;

use clap::{arg, Arg, ArgAction, Command};

// crate
use unlimited::core;
use unlimited::core::config::Config;
use unlimited::core::VERSION;
use unlimited::ui;

use unlimited::core::event::Message;

use unlimited::dbg_println;

/// Program entry point
/// It parses the command line to build the configuration.
/// Creates the core thread.
/// Nb: the created ui runs the main thread
fn main() {
    //
    //check_env_flags();

    let config = parse_command_line();

    let config_vars = config.vars.clone();

    // build core/ui communication channels
    let ui_name = config.ui_frontend.clone();
    let (ui_tx, ui_rx) = channel();
    let (core_tx, core_rx) = channel();
    // create core thread
    let core_th = start_core_thread(config.clone(), core_tx.clone(), core_rx, ui_tx.clone());

    // run the ui loop in the main thread
    ui::main_loop(&config_vars, &ui_name, &ui_rx, &ui_tx, &core_tx);

    // wait for core thread
    if let Some(core_handle) = core_th {
        core_handle.join().unwrap()
    }
}

fn start_core_thread(
    config: Config,
    core_tx: Sender<Message<'static>>,
    core_rx: Receiver<Message<'static>>,
    ui_tx: Sender<Message<'static>>,
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
// TODO(ceg): remove all special flags
// we will use a generic -c key=value
// and put them in the returned Config (as vars HashMap)
fn parse_command_line() -> Config {
    let mut fatal_error = false;

    let matches = Command::new("unlimited")
        .version(VERSION)
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("unlimited is an experimental editor")
        .arg(arg!(--ui <UI_NAME> "user interface frontend: crossterm"))
        .arg(arg!(--debug "enable debug logs on stderr (use redirection to file)"))
        .arg(
            Arg::new("LOG_FILENAME")
                .value_name("LOG_FILENAME")
                .long("log-file")
                .action(ArgAction::Append)
                .help("debug log file name (default /tmp/u.log)"),
        )
        .arg(
            Arg::new("NOREADCACHE")
                .long("no-read-cache")
                .help("disable read cache (debug)")
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("NOBYTEINDEX")
                .long("no-byte-index")
                .help("disable byte index (wip)")
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("BENCH")
                .short('b')
                .help("render all screen until EOF is reached and quit (wip: no proper quit yet)")
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("NOUIRENDER")
                .long("no-ui-render")
                .help("disable screen output")
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("RAWDATATOSCREEN")
                .short('r')
                .long("raw-data-to-screen")
                .help("disable all filters and put the file's bytes directly to screen")
                .value_parser(value_parser!(bool)),
        )
        .arg(
            Arg::new("CONFIG_VAR")
                .value_name("CONFIG_VAR")
                .short('c')
                .long("cfg-var")
                .action(ArgAction::Append)
                .help("configuration variables"),
        )
        .arg(
            arg!(<FILES> ... "file to edit")
                .required(false)
                .trailing_var_arg(true),
        )
        .get_matches();

    let log_filename = matches
        .get_one::<String>("LOG_FILENAME")
        .map_or("/tmp/u.log".to_owned(), |v| v.to_owned());

    let ui_frontend = matches
        .get_one::<String>("ui")
        .map_or("".to_owned(), |v| v.to_owned());

    let files_list = matches
        .get_many::<String>("FILES")
        .map_or(vec![], |v| v.map(|e| e.clone()).collect());

    if let Some(debug) = matches.get_one::<bool>("debug") {
        if *debug {
            core::enable_dbg_println();
            core::screen::enable_screen_checks();
        }
    }

    if matches.get_one::<bool>("NOREADCACHE").is_some() {
        core::disable_read_cache();
    }

    if matches.get_one::<bool>("NOBYTEINDEX").is_some() {
        core::disable_byte_index();
    }

    if matches.get_one::<bool>("BENCH").is_some() {
        core::enable_bench_to_eof();
    }

    if matches.get_one::<bool>("RAWDATATOSCREEN").is_some() {
        // core::enable_raw_data_filter_to_screen();
    }

    if matches.get_one::<String>("NOUIRENDER").is_some() {
        core::set_no_ui_render(true);
    }

    // configuration variables
    let mut vars: HashMap<String, String> = HashMap::new();

    match matches.get_many::<String>("CONFIG_VAR") {
        Some(s) => {
            let v = s
                .map(|x| {
                    let split: Vec<_> = x.split('=').collect();
                    if split.len() != 2 {
                        fatal_error = true;
                        eprintln!("error: invalid configuration variable: {x}");
                        ("".to_owned(), "".to_owned())
                    } else {
                        (split[0].to_owned(), split[1].to_owned())
                    }
                })
                .collect::<Vec<(String, String)>>();

            for e in v {
                if e.0.is_empty() {
                    continue;
                }
                vars.insert(e.0, e.1);
            }
        }
        _ => {}
    }

    crate::core::LOG_FILENAME.get_or_init(|| log_filename.clone());

    // debug
    dbg_println!("config vars  = \n{:?}", vars);
    dbg_println!("ui_frontend  = \n{:?}", ui_frontend);
    dbg_println!("files_list   = \n{:?}", files_list);
    dbg_println!("LOG_FILENAME = \n{:?}", log_filename);

    if fatal_error {
        std::process::exit(1);
    }

    Config {
        files_list,
        ui_frontend,
        vars,
    }
}
