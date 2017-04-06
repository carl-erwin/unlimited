//
use std::collections::HashMap;
use std::thread;

//
use core;
use ui;
use core::config::Config;
use core::buffer::BufferBuilder;
use core::buffer::Buffer;
use core::buffer;


//
pub type Id = u64;

//
pub struct Editor {
    pub config: Config,
    pub buffer_map: HashMap<buffer::Id, Box<Buffer>>,
}


impl Editor {
    ///
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

        let core_th = if self.config.start_core {
            Some(thread::spawn(move || core::start()))
        } else {
            None
        };

        if self.config.start_ui {
            ui::main_loop(self);
        }

        match core_th {
            Some(core_handle) => core_handle.join().unwrap(),
            None => {}
        }
    }

    ///
    pub fn setup_default_buffers(&mut self) {

        let b = BufferBuilder::new()
            .buffer_name("debug-message")
            .file_name("/dev/null")
            .internal(true)
            .finalize();

        self.buffer_map.insert(0, Box::new(b.unwrap()));

        let b = BufferBuilder::new()
            .buffer_name("scratch")
            .file_name("/dev/null")
            .internal(true)
            .finalize();

        self.buffer_map.insert(1, Box::new(b.unwrap()));


    }

    ///
    pub fn load_files(&mut self) {

        let mut id: u64 = 2;

        for f in &self.config.files_list {

            let b = BufferBuilder::new()
                .buffer_name(f)
                .file_name(f)
                .internal(false)
                .finalize();

            match b {
                Some(b) => {
                    self.buffer_map.insert(id, Box::new(b));
                    id += 1;
                }
                None => {}
            }

        }
    }
}
