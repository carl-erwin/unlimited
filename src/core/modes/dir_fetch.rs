use parking_lot::RwLock;
use std::rc::Rc;

use std::fs;
use std::path::PathBuf;

use crate::core::Editor;

use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

pub struct DirectoryReader {}

impl DirectoryReader {
    pub fn new() -> Self {
        DirectoryReader {}
    }
}

impl ContentFilter<'_> for DirectoryReader {
    fn name(&self) -> &'static str {
        &"DirectoryReader"
    }

    fn setup(
        &mut self,
        _editor: &Editor<'static>,
        _env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        _no_input: &[FilterIo],
        filter_out: &mut Vec<FilterIo>,
    ) {
        let buffer = view.buffer.clone();
        if let Some(ref buffer) = buffer {
            env.quit = true;

            let mut listing: Vec<String> = vec![];

            let path = buffer.read().name.clone();
            let path = PathBuf::from(path);

            match fs::read_dir(&path) {
                Ok(path) => {
                    for e in path {
                        let s = e.unwrap().path().to_str().unwrap().to_owned();
                        listing.push(s);
                    }
                }
                Err(e) => {
                    let s = format!("cannot read {:?} content {:?}\n", path, e);
                    listing.push(s);
                }
            }

            let mut raw_data: Vec<u8> = vec![];

            listing.sort();
            for s in &mut listing {
                raw_data.append(&mut s.as_bytes().to_owned());
                raw_data.push(b'\n');
            }

            let len = raw_data.len();

            (*filter_out).push(FilterIo {
                metadata: true,
                style: TextStyle::new(),
                offset: Some(0),
                size: len,
                data: FilterData::ByteArray { vec: raw_data },
            });
        }
    }
}
