extern crate clap;
extern crate unlimited;

//
use std::sync::mpsc::channel;
use std::thread;

use clap::{App, Arg};

use unlimited::core;
use unlimited::core::config::Config;

use unlimited::ui;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
    let config = parse_command_line();

    let (ui_tx, ui_rx) = channel();
    let (core_tx, core_rx) = channel();

    let ui_name = config.ui_frontend.clone();

    let core_th = { Some(thread::spawn(move || core::start(config, core_rx, ui_tx))) };

    ui::main_loop(ui_name.as_ref(), ui_rx, core_tx);

    if let Some(core_handle) = core_th {
        core_handle.join().unwrap()
    }
}

fn parse_command_line() -> Config {
    let matches = App::new("unlimited")
        .version(VERSION)
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("unlimited is an experimental editor")
        .args_from_usage("--ui, --ui=[termion|ncurses] 'select user interface fronted'")
        .arg(
            Arg::with_name("FILES")
                .help("list of the files to open")
                .required(false)
                .multiple(true),
        )
        .get_matches();

    let mut ui_frontend = String::new();
    if matches.is_present("ui") {
        let v: Vec<&str> = matches.values_of("ui").unwrap().collect();
        ui_frontend = v[0].to_owned();
    }

    let mut files_list = Vec::new();
    if matches.is_present("FILES") {
        let strs: Vec<&str> = matches.values_of("FILES").unwrap().collect();
        files_list = strs.into_iter().map(|x| x.to_owned()).collect()
    }

    Config {
        files_list,
        ui_frontend,
    }
}
