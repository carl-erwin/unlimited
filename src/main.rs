extern crate unlimited;
extern crate clap;

use clap::{App, Arg};

use unlimited::core::config::Config;
use unlimited::core::editor::Editor;

fn main() {
    let config = parse_command_line();
    let mut editor = Editor::new(config);
    editor.run();
}

fn parse_command_line() -> Config {

    let matches = App::new("unlimited")
        .version("0.0.1")
        .author("Carl-Erwin Griffith <carl.erwin@gmail.com>")
        .about("unlimited is an experimental editor")
        .arg(Arg::with_name("START_CORE")
                 .help("enable core")
                 .long("start-core"))
        .arg(Arg::with_name("NO_UI").help("disable ui").long("no-ui"))
        .arg(Arg::with_name("FILES")
                 .help("list of the files to open")
                 .required(false)
                 .multiple(true))
        .get_matches();

    let mut files_list = Vec::new();
    if matches.is_present("FILES") == true {
        let strs: Vec<&str> = matches.values_of("FILES").unwrap().collect();
        files_list = strs.into_iter().map(|x| x.to_owned()).collect()
    }

    Config {
        start_ui: !matches.is_present("NO_UI"),
        start_core: matches.is_present("START_CORE"),
        files_list,
    }
}
