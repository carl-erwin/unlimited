use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;
use crate::core::view::View;
use crate::core::Editor;
use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::codepointinfo::TextStyle;

pub struct UnicodeToTextFilter {
    cur_offset: u64,
}

impl UnicodeToTextFilter {
    pub fn new() -> Self {
        UnicodeToTextFilter { cur_offset: 0 }
    }
}

impl ContentFilter<'_> for UnicodeToTextFilter {
    fn name(&self) -> &'static str {
        &"UnicodeToTextFilter"
    }

    fn setup(
        &mut self,
        _editor: &Editor<'static>,
        env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        self.cur_offset = env.base_offset;
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &[FilterIo],
        filter_out: &mut Vec<FilterIo>,
    ) {
        for io in filter_in.iter() {
            //           dbg_println!("UNICODE TO TEXT: parsing IO {:?}", io);

            match &*io {
                FilterIo {
                    data: FilterData::UnicodeArray { vec },
                    ..
                } => {
                    filter_out.reserve(vec.len());
                    for e in vec.iter() {
                        let new_io = FilterIo {
                            // general info
                            metadata: false,
                            style: TextStyle::new(),
                            offset: Some(self.cur_offset),
                            size: e.size as usize, // count(data) ?
                            data: FilterData::TextInfo {
                                real_cp: e.cp,
                                displayed_cp: e.cp,
                            },
                            // TODO(ceg): add style infos ?
                        };

                        filter_out.push(new_io);
                        self.cur_offset += e.size as u64;
                    }

                    //                    dbg_println!("UNICODE TO TEXT : out =  {:?}", filter_out);
                }

                FilterIo {
                    data: FilterData::EndOfStream | FilterData::CustomLimitReached,
                    ..
                } => {
                    filter_out.push(io.clone());
                }

                _ => {
                    // unhandled input type -> forward
                    filter_out.push(io.clone());
                    dbg_println!("unexpected {:?}", io);
                    panic!("");
                }
            }
        }
    }
}
