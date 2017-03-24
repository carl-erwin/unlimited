extern crate unlimited;
extern crate clap;

use unlimited::core;
use unlimited::ui;

fn main() {

    // TODO:
    // parse command line arguments
    // build/load configuration

    core::start();

    // start ui
    ui::main_loop();

    core::stop();
}
