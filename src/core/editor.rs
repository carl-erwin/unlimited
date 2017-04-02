//
use std::collections::HashMap;

//
use ::core;
use ::ui;
use ::core::config::Config;

use ::core::buffer::Buffer;


pub type Id = u64; // TODO -> prefer Buffer::Id ?,

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

        for f in &self.config.files_list {
            println!("loading {}", f);
            // add to buffer_map
            // Buffer::new(filename)
        }



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
}
