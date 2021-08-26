use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;
use crate::core::view::View;
use crate::core::Editor;
use std::rc::Rc;
use std::sync::RwLock;

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

    fn setup(&mut self, _editor: &Editor, env: &mut LayoutEnv, _view: &Rc<RwLock<View>>) {
        self.cur_offset = env.base_offset;
    }

    fn run(
        &mut self,
        _view: &View,
        _env: &mut LayoutEnv,
        filter_in: &Vec<FilterIo>,
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
                    data: FilterData::EndOfStream | FilterData::StreamLimitReached,
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
