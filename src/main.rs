extern crate clap;
extern crate unlimited;

use clap::{App, Arg};

use unlimited::core::config::Config;
use unlimited::core::editor::Editor;

const VERSION: &'static str = env!("CARGO_PKG_VERSION");

fn main() {
    let config = parse_command_line();
    let mut editor = Editor::new(config);
    editor.run();
}

fn parse_command_line() -> Config {
    let matches = App::new("unlimited")
        .version(VERSION)
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("unlimited is an experimental editor")
        .arg(
            Arg::with_name("NO_CORE")
                .help("disable core")
                .long("no-core"),
        )
        .arg(Arg::with_name("NO_UI").help("disable ui").long("no-ui"))
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
        start_ui: !matches.is_present("NO_UI"),
        start_core: !matches.is_present("NO_CORE"),
        files_list,
        ui_frontend,
    }
}
