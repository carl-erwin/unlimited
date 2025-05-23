extern crate unicode_width;

use unicode_width::UnicodeWidthChar;

use crate::core::codepointinfo::CodepointInfo;
use crate::core::codepointinfo::TextStyle;

use std::sync::atomic::{AtomicUsize, Ordering};

pub static SCREEN_CHECK_FLAG: AtomicUsize = AtomicUsize::new(0);

pub fn enable_screen_checks() {
    SCREEN_CHECK_FLAG.store(1, Ordering::Relaxed);
}
pub fn disable_screen_checks() {
    SCREEN_CHECK_FLAG.store(0, Ordering::Relaxed);
}
pub fn toggle_screen_checks() {
    let v = SCREEN_CHECK_FLAG.load(Ordering::Relaxed);
    SCREEN_CHECK_FLAG.store(!v, Ordering::Relaxed);
}

/// A LineCell encapsulates code point information (CodepoinInfo).<br/>
/// The displayed Lines are composed of LineCell
#[derive(Hash, Default, Debug, Clone, Eq, PartialEq)]
pub struct ScreenCell {
    pub cpi: CodepointInfo,
}

impl ScreenCell {
    pub fn new() -> Self {
        ScreenCell {
            cpi: CodepointInfo::new(),
        }
    }
}

/// A Screen is composed of width*height ScreenCell(s).<br/>
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Screen {
    /// the underlying lines storage
    pub buffer: Vec<ScreenCell>,
    /// the number of elements pushed to the screen
    pub push_count: usize,
    // the maximum number of elements the screen can hold
    pub push_capacity: usize,

    current_line_index: usize,
    current_line_remain: usize,

    pub is_off_screen: bool, // Hints
    pub has_eof: bool,       // Hints
    is_start_of_line: bool,

    /// maximum number of elements the screen line can hold
    width: usize,
    /// maximum number of lines the screen can hold
    height: usize,
    /// placeholder to record the offset of the first pushed CodepointInfo (used by View)
    pub first_offset: Option<u64>,
    /// placeholder to record the offset of the last pushed CodepointInfo (used by View)
    pub last_offset: Option<u64>,

    /// placeholder to record the maximum offset of the buffer (eof)
    pub buffer_max_offset: u64,

    pub line_offset: Vec<(u64, u64)>,
    pub line_index: Vec<(usize, usize)>,

    pub style: TextStyle,
}

impl Screen {
    pub fn with_dimension(dim: (usize, usize)) -> Screen {
        Screen::new(dim.0, dim.1)
    }

    pub fn dimension(&self) -> (usize, usize) {
        (self.width(), self.height())
    }

    pub fn new(width: usize, height: usize) -> Screen {
        let push_capacity = width * height;
        let mut buffer: Vec<ScreenCell> = vec![];
        buffer.resize(push_capacity, ScreenCell::new());
        Screen {
            is_off_screen: false,
            has_eof: false,
            is_start_of_line: true,
            buffer,
            current_line_index: 0,
            current_line_remain: width,
            width,
            height,
            push_count: 0,
            push_capacity,
            first_offset: None,
            last_offset: None,
            buffer_max_offset: 0,
            line_offset: Vec::with_capacity(height),
            line_index: Vec::with_capacity(height),
            style: TextStyle::new(),
        }
    }

    pub fn check_invariants(&self) {
        if self.push_count == 0 {
            return;
        }

        if SCREEN_CHECK_FLAG.load(Ordering::Relaxed) == 0 {
            return;
        }

        if self.last_offset.is_none() {
            return;
        }
        if self.first_offset.is_none() {
            return;
        }

        let mut cur_offset = self.first_offset;

        if let (Some(cur), Some(first)) = (cur_offset, self.buffer[0].cpi.offset) {
            assert_eq!(cur, first)
        }

        if self.has_eof() {
            // last_offset += 1;
        }

        for cell in &self.buffer {
            if !cell.cpi.used {
                continue;
            }

            if let (Some(cur), Some(cpi_offset)) = (cur_offset, cell.cpi.offset) {
                if cpi_offset < cur {
                    panic!("cpi_offset {} < cur {}", cpi_offset, cur);
                }
                cur_offset = Some(cpi_offset);
            }
        }
    }

    pub fn copy_screen_at_xy(&mut self, src: &Screen, x: usize, y: usize) -> bool {
        for src_y in 0..src.height() {
            for src_x in 0..src.width() {
                if let Some(cpi_src) = src.get_cpinfo(src_x, src_y) {
                    if let Some(cpi_dst) = self.get_cpinfo_mut(x + src_x, y + src_y) {
                        *cpi_dst = *cpi_src;
                    }
                }
            }
        }

        true
    }

    pub fn width(&self) -> usize {
        self.width
    }

    pub fn height(&self) -> usize {
        self.height
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.push_capacity = width * height;

        let old_len = self.buffer.len();
        self.buffer.resize(self.push_capacity, ScreenCell::new());
        for e in self.buffer.iter_mut().take(old_len) {
            *e = ScreenCell::new();
        }

        self.height = height;
        self.width = width;

        self.has_eof = false;
        self.is_start_of_line = true;
        self.current_line_index = 0;
        self.current_line_remain = width;

        self.push_count = 0;
        self.first_offset = None;
        self.last_offset = None;
        self.buffer_max_offset = 0;

        self.line_offset.clear();
        self.line_index.clear();
    }

    pub fn push_available(&self) -> usize {
        if self.push_capacity() >= self.push_count {
            self.push_capacity() - self.push_count
        } else {
            0
        }
    }

    pub fn push_count(&self) -> usize {
        self.push_count
    }

    pub fn push_capacity(&self) -> usize {
        self.push_capacity
    }

    pub fn current_line_index(&mut self) -> usize {
        self.current_line_index
    }

    pub fn select_next_line_index(&mut self) -> usize {
        if self.current_line_index == self.height {
            return self.current_line_index;
        }

        let used = self.width - self.current_line_remain;
        let skip = self.width - used;

        self.push_count += skip;
        self.current_line_index += 1; // go to next line
        self.current_line_remain = self.width;

        self.is_start_of_line = true;

        self.current_line_index
    }

    /// 0-----skip---cur_index----max_height---capacity

    #[inline(always)]
    pub fn char_width(&self, c: char) -> usize {
        UnicodeWidthChar::width(c).unwrap_or(1)
    }

    /// will use self.style
    pub fn push_char(&mut self, c: char) -> (bool, usize) {
        let mut cpi = CodepointInfo::new();
        cpi.cp = c;
        cpi.displayed_cp = c;
        cpi.style = self.style;
        self.push(&cpi)
    }

    /// Append CodepoinInfo tu the current line if it fits.
    /// filters basic blanks '\r' '\n' '\t' 0x0..0x1f and replace them by space ' '
    //#[inline(always)]
    pub fn push(&mut self, cpi: &CodepointInfo) -> (bool, usize) {
        // self.check_invariants();

        // internal cpi
        let mut cpi = *cpi;

        // TODO(ceg):  cpi.check_invariants();
        if false {
            if !cpi.metadata && cpi.size == 0 {
                dbg_println!("CPI = {:?}", cpi);
                panic!();
            }
            if cpi.metadata && cpi.size != 0 {
                dbg_println!("CPI = {:?}", cpi);
                panic!();
            }
        }

        // check cpi
        let mut line_feed = false;

        let mut unicode_width = self.char_width(cpi.cp);
        if unicode_width == 0 {
            unicode_width = 1;
            if cpi.displayed_cp == cpi.cp {
                cpi.displayed_cp = '.';
            }
        } else {
            // do not allow invisible char
            // no char replacement ?
            if cpi.cp == cpi.displayed_cp {
                match cpi.cp {
                    '\n' => {
                        line_feed = true;
                        cpi.displayed_cp = ' ';
                    }
                    '\t' => {
                        cpi.displayed_cp = ' '; // '\u{2b7e}'; not supported
                    }
                    '\r' => {
                        cpi.displayed_cp = '\u{2190}';
                    }
                    '\u{ad}' => {
                        cpi.displayed_cp = '.';
                    }
                    '\u{7f}'..='\u{9f}' => {
                        cpi.displayed_cp = '.';
                    }
                    '\u{0}'..='\u{1f}' => {
                        cpi.displayed_cp = '.';
                    }

                    _ => {}
                }
            }

            // filter new line/carriage return
            if cpi.cp == '\n' || cpi.displayed_cp == '\n' {
                cpi.displayed_cp = ' ';
                line_feed = true;
            } else if cpi.cp == '\r' {
                cpi.displayed_cp = ' ';
            }

            assert!(cpi.displayed_cp >= ' ');
        }

        // does char fit ?
        if unicode_width > self.current_line_remain {
            self.select_next_line_index();
        }

        // screen.is_full() ? all line used ?
        if self.current_line_index == self.height() {
            return (false, self.current_line_index);
        }

        // update line info: move to self.finalize()

        // new line start
        if self.is_start_of_line {
            // update current line info :offset/index
            if let Some(off) = cpi.offset {
                let offset_pair = (off, off);
                let index_pair = (self.push_count, self.push_count);
                self.line_offset.push(offset_pair);
                self.line_index.push(index_pair);
                self.is_start_of_line = false; // here ? what happens if we push meta char with no offset ?
            }
        }

        if self.line_offset.is_empty() {
            if let Some(off) = self.first_offset {
                self.line_offset.push((off, off));
                self.line_index.push((self.push_count, self.push_count));
            }
        }

        if self.push_count == 0 {
            self.first_offset = cpi.offset;
        } else {
            let cpi_offset = cpi.offset;
            let last_offset = self.last_offset;

            if let (Some(cpi_offset), Some(last_offset)) = (cpi_offset, last_offset) {
                // check invariants
                if true {
                    if cpi_offset < last_offset {
                        dbg_println!(
                            "cpi {:?} , cpi_offset {:?} < last_offset {:?}",
                            cpi,
                            cpi_offset,
                            last_offset
                        );

                        panic!(); // allow unsorted offsets ?
                    }
                    if false {
                        dbg_println!(
                            "SAVE/UPDATE line[{}] end @ offset {} , index {}",
                            self.current_line_index,
                            cpi_offset,
                            self.push_count
                        );
                    }
                }

                // save end line info
                self.line_offset.last_mut().unwrap().1 = cpi_offset;
                self.line_index.last_mut().unwrap().1 = self.push_count;
            }
        }

        unsafe {
            //self.buffer[self.push_count].cpi = cpi;
            let cell = self.buffer.get_unchecked_mut(self.push_count);
            cell.cpi = cpi;

            for i in 1..unicode_width {
                //self.buffer[self.push_count + i].cpi = cpi;
                //self.buffer[self.push_count + i].cpi.skip_render = true;

                let cell = self.buffer.get_unchecked_mut(self.push_count + i);
                cell.cpi = cpi;
                cell.cpi.skip_render = true;
            }
        }

        self.last_offset = cpi.offset;

        self.push_count += unicode_width;
        self.current_line_remain -= unicode_width; // TODO(ceg): saturating sub here ?
        if line_feed || self.current_line_remain == 0 {
            self.select_next_line_index();
        }

        (true, self.current_line_index)
    }

    // call this to compute line start/end offsets, line_indexes
    // TODO: move all push(..) code that deal with metadata
    pub fn finalize(&mut self) {}

    pub fn append(&mut self, cpi_vec: &Vec<CodepointInfo>) -> (usize, usize, Option<u64>) {
        for (idx, cpi) in cpi_vec.iter().enumerate() {
            let ret = self.push(&cpi);
            if !ret.0 {
                // cannot push screen full
                return (idx, ret.1, self.last_offset);
            }
        }

        (cpi_vec.len(), self.current_line_index, self.last_offset)
    }

    pub fn clear(&mut self) {
        // reset cells
        for cell in self.buffer.iter_mut() {
            *cell = ScreenCell::new();
        }

        self.is_off_screen = false;
        self.has_eof = false;
        self.is_start_of_line = true;

        self.current_line_index = 0;
        self.current_line_remain = self.width;
        self.push_count = 0;

        self.first_offset = None;
        self.last_offset = None;
        self.buffer_max_offset = 0; // TODO(ceg): Option<u64>

        self.line_offset.clear();
        self.line_index.clear();
    }

    pub fn has_eof(&self) -> bool {
        self.has_eof
    }

    pub fn set_has_eof(&mut self) {
        self.has_eof = true;
    }

    pub fn get_last_used_line_index(&self) -> Option<usize> {
        if self.line_index.is_empty() {
            return None;
        }
        Some(self.line_index.len() - 1)
    }

    pub fn get_line_mut(&mut self, index: usize) -> Option<&mut [ScreenCell]> {
        if index >= self.height() {
            dbg_println!(
                "index {} >= self.height {} (self.width {})",
                index,
                self.height,
                self.width
            );
            return None;
        }

        let start = self.width * index;
        let end = start + self.width;
        Some(&mut self.buffer[start..end])
    }

    pub fn get_line(&self, index: usize) -> Option<&[ScreenCell]> {
        if index >= self.height() {
            dbg_println!("index {} >= self.height {}", index, self.height);
            return None;
        }

        let start = self.width * index;
        let end = start + self.width;
        Some(&self.buffer[start..end])
    }

    pub fn get_used_line_mut(&mut self, index: usize) -> Option<&mut [ScreenCell]> {
        if index >= self.line_index.len() {
            return None;
        }

        let (start, end) = self.line_index[index];
        Some(&mut self.buffer[start..end + 1])
    }

    pub fn get_used_line(&self, index: usize) -> Option<&[ScreenCell]> {
        if index >= self.line_index.len() {
            return None;
        }

        let (start, end) = self.line_index[index];
        Some(&self.buffer[start..end + 1])
    }

    pub fn get_first_used_line(&self) -> Option<&[ScreenCell]> {
        if self.push_count() == 0 {
            return None;
        }

        let p = &self.line_index[0];
        Some(&self.buffer[p.0..p.1 + 1])
    }

    pub fn get_last_used_line(&self) -> Option<&[ScreenCell]> {
        if self.push_count() == 0 {
            dbg_println!("push_count = {:?}", self.push_count);
            return None;
        }

        let p = self.line_index.last().unwrap();
        dbg_println!("range  = {:?}", p);

        Some(&self.buffer[p.0..p.1 + 1])
    }

    pub fn get_cpinfo(&self, x: usize, y: usize) -> Option<&CodepointInfo> {
        let l = self.get_line(y)?;
        if x >= l.len() {
            return None;
        }

        Some(&l[x].cpi)
    }

    pub fn get_cpinfo_mut(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        let l = self.get_line_mut(y)?;
        if x >= l.len() {
            return None;
        }

        Some(&mut l[x].cpi)
    }

    pub fn at_xy_mut(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        self.get_cpinfo_mut(x, y)
    }

    pub fn at_xy(&self, x: usize, y: usize) -> Option<&CodepointInfo> {
        self.get_cpinfo(x, y)
    }

    pub fn find_cpi_by_offset(&self, offset: u64) -> (Option<&CodepointInfo>, usize, usize) {
        for idx in 0..self.push_count {
            let cpi = &self.buffer[idx].cpi;
            if let Some(cpi_off) = cpi.offset {
                if cpi_off >= offset && offset <= cpi_off + cpi.size as u64 {
                    let x = idx % self.width;
                    let y = idx / self.width;
                    return (Some(cpi), x, y);
                }
            }
        }

        (None, 0, 0)
    }

    pub fn contains_offset(&self, offset: u64) -> bool {
        if self.first_offset.is_none() || self.last_offset.is_none() {
            return false;
        }

        let first_offset = self.first_offset.unwrap();
        let last_offset = self.last_offset.unwrap();

        if offset < first_offset || offset > last_offset {
            return false;
        }

        dbg_println!("find cpi by offset {}", offset);
        let (cpi, _, _) = self.find_cpi_by_offset(offset);
        cpi.is_some()
    }
}

pub fn screen_apply<F: FnMut(usize, usize, &mut CodepointInfo) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            for (c, cell) in line.iter_mut().enumerate() {
                let cpi = &mut cell.cpi;
                if !on_cpi(c, l, cpi) {
                    return;
                }
            }
        }
    }
}

pub fn screen_line_apply<F: FnMut(usize, &mut [ScreenCell]) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            if !on_cpi(l, line) {
                return;
            }
        }
    }
}

// TODO(ceg): test are broken
#[test]
fn test_screen() {}
