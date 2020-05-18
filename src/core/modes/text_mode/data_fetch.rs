use crate::core::view::layout::ContentFilter;
use crate::core::view::layout::FilterData;
use crate::core::view::layout::FilterIo;
use crate::core::view::layout::LayoutEnv;

use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

pub struct RawDataFilter {
    // data
    pos: u64,
    read_max: usize,
    read_size: usize,
    read_count: usize,
}

impl RawDataFilter {
    pub fn new() -> Self {
        RawDataFilter {
            pos: 0,
            read_max: 0,
            read_size: 0,
            read_count: 0,
        }
    }
}

impl ContentFilter<'_> for RawDataFilter {
    fn name(&self) -> &'static str {
        &"RawDataFilter"
    }

    fn setup(&mut self, env: &mut LayoutEnv, _view: &View) {
        dbg_println!(
            "RawDataFilter w {} h {}",
            env.screen.width(),
            env.screen.height()
        );

        self.read_max = env.screen.width() * env.screen.height() * 4; // 4: max utf8 encode size
        self.read_count = 0;
        self.read_size = 1024 * 8 * 3; // env ?
                                       //self.read_size = (env.screen.width() * env.screen.height();

        self.pos = env.base_offset;
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        _noinput: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        // There is no input HERE
        // we convert the document data into  buffer FilterIo

        // we read screen.width() bytes // TODO: width * codec_max_encode_size() for now
        let doc = view.document.clone();
        if let Some(ref doc) = doc {
            // 1st pass raw_data_filter
            let mut raw_data = vec![];

            if self.read_count + self.read_size > self.read_max {
                self.read_size = self.read_max - self.read_count;
                env.quit = true;
            }

            let rd = doc
                .read()
                .unwrap()
                .read(self.pos, self.read_size, &mut raw_data);

            /*
                        dbg_println!(
                            "READ from offset({}) : {} / {} bytes",
                            self.pos,
                            rd,
                            self.read_size
                        );
                        dbg_println!("BUFFER SIZE {}", doc.read().unwrap().size());
                        dbg_println!("POS {} + RD {}  = {}", self.pos, rd, self.pos + rd as u64);
            */
            if rd > 0 {
                (*filter_out).push(FilterIo {
                    metadata: false,
                    style: TextStyle::new(),
                    offset: Some(self.pos),
                    size: rd,
                    data: FilterData::ByteArray { vec: raw_data },
                });
            }

            if rd < self.read_size {
                env.quit = true;

                (*filter_out).push(FilterIo {
                    metadata: true,
                    style: TextStyle::new(),
                    offset: Some(self.pos + rd as u64),
                    size: 0,
                    data: FilterData::EndOfStream,
                });

                dbg_println!("EOF @ offset {}", self.pos + rd as u64);
            }

            self.pos += rd as u64;
            self.read_count += rd;

            // increase read size at every call
            // TODO: find better default size
            if self.read_size < 1024 * 1024 {
                self.read_size += env.screen.width(); // TODO: enable this
            }
        }
    }
}
