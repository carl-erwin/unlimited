//
use std::rc::Rc;
use std::collections::HashMap;
use std::thread;


//
use core;
use ui;
use core::config::Config;
use core::buffer::BufferBuilder;
use core::buffer::Buffer;
use core::buffer;

use core::view::View;
use core::view;



//
pub type Id = u64;

//
/* Hierarchy Reminder

    core
        Editor
            config
            buffer_map<Rc<buffer>>
            view_map
            Option<&view>
                &buffer
                list<mode>
                input_map

    ui
        (vid, bid)

*/
pub struct Editor<'a> {
    pub config: Config,
    pub buffer_map: HashMap<buffer::Id, Rc<Buffer>>,
    pub view_map: HashMap<view::Id, Box<View>>,
    pub view: Option<&'a View>,
}


impl<'a> Editor<'a> {
    ///
    pub fn new(config: Config) -> Editor<'a> {
        Editor {
            config: config,
            buffer_map: HashMap::new(),
            view_map: HashMap::new(),
            view: None,
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

        self.buffer_map.insert(0, b.unwrap());

        let b = BufferBuilder::new()
            .buffer_name("scratch")
            .file_name("/dev/null")
            .internal(true)
            .finalize();

        self.buffer_map.insert(1, b.unwrap());
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
                    self.buffer_map.insert(id, b);
                    id += 1;
                }
                None => {}
            }
        }
    }
}
