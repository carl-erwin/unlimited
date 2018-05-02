//
use std::rc::Rc;
use std::cell::RefCell;
use std::collections::HashMap;

use std::convert::AsRef;

//
use std::thread;
use std::sync::mpsc::channel;
use std::sync::mpsc::Sender;
use std::sync::mpsc::Receiver;

//
use core;
use ui;
use core::config::Config;

use core::document::DocumentBuilder;
use core::document::Document;
use core::document;

use core::view::View;
use core::view;

use core::event::Event;

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
    }
}
