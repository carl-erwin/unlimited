// Copyright (c) Carl-Erwin Griffith
//
// Permission is hereby granted, free of charge, to any
// person obtaining a copy of this software and associated
// documentation files (the "Software"), to deal in the
// Software without restriction, including without
// limitation the rights to use, copy, modify, merge,
// publish, distribute, sublicense, and/or sell copies of
// the Software, and to permit persons to whom the Software
// is furnished to do so, subject to the following
// conditions:
//
// The above copyright notice and this permission notice
// shall be included in all copies or substantial portions
// of the Software.
//
// THE SOFTWARE IS PROVIDED "AS IS", WITHOUT WARRANTY OF
// ANY KIND, EXPRESS OR IMPLIED, INCLUDING BUT NOT LIMITED
// TO THE WARRANTIES OF MERCHANTABILITY, FITNESS FOR A
// PARTICULAR PURPOSE AND NONINFRINGEMENT. IN NO EVENT
// SHALL THE AUTHORS OR COPYRIGHT HOLDERS BE LIABLE FOR ANY
// CLAIM, DAMAGES OR OTHER LIABILITY, WHETHER IN AN ACTION
// OF CONTRACT, TORT OR OTHERWISE, ARISING FROM, OUT OF OR
// IN CONNECTION WITH THE SOFTWARE OR THE USE OR OTHER

//
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

//
use core::config::Config;

use core::document;
use core::document::Document;
use core::document::DocumentBuilder;

use core::view;
use core::view::View;

//
pub type Id = u64;

//
/* Hierarchy Reminder

    core
        editor
            config
            document_map<doc_id, Rc<Document>>
            view_map<view_id, Rc<View>>

    TODO:
            Option<&view>
                &buffer
                list<mode>
                input_map

    ui
        (vid, bid)

*/

/* TODO:
    parse argument to extract line,colinfo,offset
    file@1246
    file:10
    file:10,5
    +l file
    +l,c file
    @offset file

    document_list: Vec<
        struct DocumentInfo {
            FileType: { directory, regular, internal }
            relative_path,: String  test,        *debug-message*
            real_path: String : /home/user/test, /dev/null
            id,
            special_file : bool,
            internal_document : bool,
            start_line: usize
            start_column: usize
            start_offset
        }
    >


    keep user argument order, push new files,
    this list is never cleared
    before insertion the realpath is checked to avoid double open

    document_index: HashMap<String, document::Id>,  document::Id is the position in document_list

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
            config,
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

    /// TODO: replace this by load/unload doc functions
    /// the ui will open the documents on demand
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
