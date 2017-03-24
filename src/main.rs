extern crate unlimited;
extern crate clap;

use clap::{App, Arg};

use unlimited::core;
use unlimited::ui;

fn main() {
    let config = parse_command_line();

    if config.start_core {
        core::start();
    }

    if config.start_ui {
        ui::main_loop();
    }

    if config.start_core {
        core::stop();
    }
}


// TODO:
// parse command line arguments
// build/load configuration
fn parse_command_line() -> core::config::Config {

    let matches = App::new("unlimited")
        .version("0.0.1")
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("an experimental editor")
        .arg(Arg::with_name("NO_CORE")
            .help("disable core")
            .long("no-core"))
        .arg(Arg::with_name("NO_UI")
            .help("disable ui")
            .long("no-ui"))
        .arg(Arg::with_name("FILES")
            .help("Sets the input file to use")
            .required(false)
            .multiple(true))
        .get_matches();


    let mut file_list = Vec::new();
    if matches.is_present("FILES") == true {
        let strs: Vec<&str> = matches.values_of("FILES").unwrap().collect();
        file_list = strs.into_iter().map(|x| x.to_owned()).collect()
    }

    core::config::Config {
        start_core: !matches.is_present("NO_CORE"),
        start_ui: !matches.is_present("NO_UI"),
        file_list: file_list,
    }
}
