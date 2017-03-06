extern crate unlimited;

fn main() {

    // TODO:
    // parse command line arguments
    // build/load configuration

    // start core
    unlimited::core::start();

    // start ui
    unlimited::ui::main_loop();
}
