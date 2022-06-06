use parking_lot::RwLock;
use std::rc::Rc;

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
        if let Some(ref _buffer) = buffer {
            env.quit = true;

            let raw_data = "Directory listing not implemented yet !\n"
                .as_bytes()
                .to_owned();
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
