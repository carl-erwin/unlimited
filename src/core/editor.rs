//
use std::collections::HashMap;

//
use core;
use ui;
use core::config::Config;

use core::buffer::*;


pub type Id = u64;

//
pub struct Editor {
    pub config: Config,
    pub buffer_map: HashMap<Id, Box<Buffer>>,
}


impl Editor {
    pub fn new(config: Config) -> Editor {
        Editor {
            config: config,
            buffer_map: HashMap::new(),
        }
    }

    /// load files/buffers/etc ...
    pub fn run(&mut self) {

        self.setup_default_buffers();

        self.load_files();

        if self.config.start_core {
            core::start();
        }

        if self.config.start_ui {
            ui::main_loop();
        }

        if self.config.start_core {
            core::stop();
        }
    }

    pub fn setup_default_buffers(&mut self) {

        //let b =
        BufferBuilder::new()
            .buffer_name("scratch")
            .internal(true)
            .finalize();

        // self.add_buffer(b);
    }

    pub fn load_files(&mut self) {
        for f in &self.config.files_list {
            println!("checking '{}'", f);
            // add to buffer_map
            // Buffer::new(filename)
        }
    }
}
