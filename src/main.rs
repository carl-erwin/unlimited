// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

extern crate clap;
extern crate unlimited;

//
use std::sync::mpsc::channel;
use std::thread;

use clap::{App, Arg};

use unlimited::core;
use unlimited::core::config::Config;
use unlimited::core::VERSION;
use unlimited::ui;

/// program entry point
/// It parses the command line to build the configuration.
/// Creates the core thread.
/// Nb: the ui is kept in the main thread
fn main() {
    // parse user command line
    let config = parse_command_line();

    // build core/ui communication channels
    let (ui_tx, ui_rx) = channel();
    let (cr_tx, cr_rx) = channel();
    let ui_name = config.ui_frontend.clone();

    // create core thread
    let core_th = {
        let ui_tx_clone = ui_tx.clone();
        Some(thread::spawn(move || {
            core::start(config, &cr_rx, &ui_tx_clone)
        }))
    };

    // run the ui loop in the main thread
    ui::main_loop(ui_name.as_ref(), &ui_rx, &ui_tx, &cr_tx);

    // wait for core thread
    if let Some(core_handle) = core_th {
        core_handle.join().unwrap()
    }
}

fn parse_command_line() -> Config {
    let matches = App::new("unlimited")
        .version(VERSION)
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("unlimited is an experimental editor")
        .args_from_usage("--ui, --ui=[termion|ncurses] 'select user interface frontend'")
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

    Config {
        files_list,
        ui_frontend,
    }
}
