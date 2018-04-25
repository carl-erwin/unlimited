//
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;
use std::thread;

//
use core;
use ui;
use core::config::Config;

use core::document::DocumentBuilder;
use core::document::Document;
use core::document;

use core::view::View;
use core::view;

//
pub type Id = u64;

//
/* Hierarchy Reminder

    core
        Editor
            config
            document_map<Rc<buffer>>
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
    pub document_map: HashMap<document::Id, Rc<RefCell<Document<'a>>>>,
    pub view_map: Vec<(view::Id, Rc<RefCell<View<'a>>>)>,
}

impl<'a> Editor<'a> {
    ///
    pub fn new(config: Config) -> Editor<'a> {
        Editor {
            config: config,
            document_map: HashMap::new(),
            view_map: Vec::new(),
        }
    }

    /// load files/buffers/etc ...
    pub fn run(&mut self) {
        if 0 == 1 {
            self.setup_default_buffers();
        }

        self.load_files();

        let core_th = if self.config.start_core {
            Some(thread::spawn(move || core::start()))
        } else {
            None
        };

        if self.config.start_ui {
            ui::main_loop(self);
        }

        if let Some(core_handle) = core_th {
            core_handle.join().unwrap()
        }
    }

    ///
    pub fn setup_default_buffers(&mut self) {
        let builder = DocumentBuilder::new()
            .document_name("debug-message")
            .file_name("/dev/null")
            .internal(true);

        let b = builder.finalize();

        if let Some(b) = b {
            let id = self.document_map.len() as u64;
            self.document_map.insert(id, b);
        }

        let builder = DocumentBuilder::new()
            .document_name("scratch")
            .file_name("/dev/null")
            .internal(true);

        let b = builder.finalize();

        if let Some(b) = b {
            let id = self.document_map.len() as u64;
            self.document_map.insert(id, b);
        }
    }

    ///
    pub fn load_files(&mut self) {
        let mut id = self.document_map.len() as u64;

        for f in &self.config.files_list {
            let b = DocumentBuilder::new()
                .document_name(f)
                .file_name(f)
                .internal(false)
                .finalize();

            if let Some(b) = b {
                self.document_map.insert(id, b);
                id += 1;
            }
        }

        // default buffer ?
        if self.document_map.is_empty() {
            // edit.get_untitled_count() -> 1

            let b = DocumentBuilder::new()
                .document_name("untitled-1")
                .file_name("/dev/null")
                .internal(false)
                .finalize();
            if let Some(b) = b {
                self.document_map.insert(id, b);
            }
        }

        println!("nb docs = {}", self.document_map.len());
    }
}
