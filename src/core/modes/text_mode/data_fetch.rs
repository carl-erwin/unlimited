use parking_lot::RwLock;
use std::rc::Rc;

use crate::core::Editor;

use crate::core::view::ContentFilter;
use crate::core::view::FilterData;
use crate::core::view::FilterIo;
use crate::core::view::LayoutEnv;

use crate::core::codepointinfo::TextStyle;

use crate::core::view::View;

use crate::core::bench_to_eof;

pub struct RawDataFilter {
    // data
    debug: bool,
    pos: u64,
    max_pos: u64,
    read_max: usize,
    read_size: usize,
    read_count: usize,
}

impl RawDataFilter {
    pub fn new() -> Self {
        RawDataFilter {
            debug: !true,
            pos: 0,
            max_pos: 0,
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

    fn setup(
        &mut self,
        _editor: &Editor,
        env: &mut LayoutEnv,
        _view: &Rc<RwLock<View>>,
        _parent_view: Option<&View<'static>>,
    ) {
        if self.debug {
            dbg_println!(
                "RawDataFilter w {} h {}",
                env.screen.width(),
                env.screen.height()
            );
        }

        self.pos = env.base_offset;
        self.max_pos = env.max_offset;

        //
        self.read_count = 0;
        self.read_max = env.screen.width() * env.screen.height() * 4;
        self.read_size = env.screen.width(); // * env.screen.height() / 4; // 4: max utf8 encode size

        if bench_to_eof() {
            let bench_size = 1024 * 32;
            self.read_max = (self.max_pos - self.pos) as usize;
            self.read_max = std::cmp::min(bench_size, self.read_max);
            self.read_size = self.read_max;
        }

        if self.debug {
            dbg_println!("DATA FETCH self.pos         = {}", self.pos);
            dbg_println!("DATA FETCH self.max_pos     = {}", self.max_pos);
            dbg_println!("DATA FETCH self.read_size   = {}", self.read_size);
            dbg_println!("DATA FETCH diff max_pos pos = {}", self.max_pos - self.pos);
            dbg_println!("DATA FETCH diff max_pos pos = {}", self.max_pos - self.pos);
        }
    }

    fn run(
        &mut self,
        view: &View,
        env: &mut LayoutEnv,
        _no_input: &Vec<FilterIo>,
        filter_out: &mut Vec<FilterIo>,
    ) {
        // dbg_println!("DATA FETCH: run -----------------------");
        // There is no input HERE
        // we convert the document data into  buffer FilterIo

        // we read screen.width() bytes // TODO(ceg): width * codec_max_encode_size() for now
        let doc = view.document.clone();
        if let Some(ref doc) = doc {
            // 1st pass raw_data_filter
            let mut raw_data = vec![]; // TODO(ceg): write directly to next filter input

            // disable this block to test until eof
            if !bench_to_eof() {
                if self.pos + self.read_size as u64 > self.max_pos {
                    self.read_size = self.max_pos.saturating_sub(self.pos) as usize;
                    if self.debug {
                        dbg_println!(
                            "DATA FETCH: pos + rd_size > max_pos adjust read_size : {}",
                            self.read_size
                        );
                    }
                }

                if self.debug {
                    dbg_println!(
                        "DATA FETCH: remaining bytes to read = {}",
                        self.max_pos.saturating_sub(self.pos)
                    );
                }
            }

            if self.debug {
                dbg_println!("DATA FETCH: read_size  = {}", self.read_size);
            }

            if self.debug {
                dbg_println!(
                    "DATA FETCH: try READ from offset({}) {} bytes",
                    self.pos,
                    self.read_size
                );
            }

            //
            let doc = doc.read();

            let rd = doc.read(self.pos, self.read_size, &mut raw_data);

            if self.debug {
                dbg_println!(
                    "DATA FETCH: READ from offset({}) : {} / {} bytes",
                    self.pos,
                    rd,
                    self.read_size
                );
                dbg_println!("DATA FETCH: BUFFER SIZE {}", doc.size());
                dbg_println!(
                    "DATA FETCH: POS {} + RD {}  = {}",
                    self.pos,
                    rd,
                    self.pos + rd as u64
                );
            }
            if false {
                let mut count = 0;
                for i in 0..raw_data.len() {
                    count = count % env.screen.width();
                    if count == 0 {}
                    if raw_data[i] == b'\n' {
                        count = 0;
                        dbg_println!("[ offset {} ] -- LF\n", self.pos + i as u64);
                        continue;
                    } else {
                        dbg_print!(
                            "[ offset {} ] = 0x{:x}\n",
                            self.pos + i as u64,
                            raw_data[i] as usize
                        );
                    }
                    count += 1;
                }
                dbg_println!("");
            }

            if rd > 0 {
                if self.debug {
                    dbg_println!("build byte chunk from offset self.pos {}", self.pos);
                }

                (*filter_out).push(FilterIo {
                    metadata: false,
                    style: TextStyle::new(),
                    offset: Some(self.pos),
                    size: rd,
                    data: FilterData::ByteArray { vec: raw_data },
                });
            }

            if self.debug {
                dbg_println!(
                    "updated POS {} + rd {} = {}",
                    self.pos,
                    rd,
                    self.pos + (rd as u64)
                );
            }

            self.pos += rd as u64;
            self.read_count += rd;

            // TODO(ceg): cache doc size ?
            if self.pos == doc.size() as u64 {
                env.quit = true;

                if true {
                    (*filter_out).push(FilterIo {
                        metadata: true,
                        style: TextStyle::new(),
                        offset: Some(self.pos),
                        size: 0,
                        data: FilterData::EndOfStream,
                    });
                }
                if self.debug {
                    dbg_println!("DATA FETCH: EOF @ offset {}", self.pos);
                }
            }

            if !bench_to_eof() {
                // TODO(ceg): in setup : self.read_to_eof = bench_to_eof()
                if self.pos >= self.max_pos {
                    env.quit = true;
                    if self.debug {
                        dbg_println!("DATA FETCH: MAX POS REACHED");
                    }
                    (*filter_out).push(FilterIo {
                        metadata: true,
                        style: TextStyle::new(),
                        offset: Some(self.pos),
                        size: 0,
                        data: FilterData::StreamLimitReached,
                    });
                }
            }

            // increase read size at every call
            // TODO(ceg): find better default size
            if self.read_size < self.read_max {
                self.read_size += env.screen.width();
                //dbg_println!("updated read size  {} ", self.read_size);
            }
        }
    }
}
