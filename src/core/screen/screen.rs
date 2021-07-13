extern crate unicode_width;

use unicode_width::UnicodeWidthChar;

use super::line::Line;
use super::line::LineCellIndex;
use crate::core::codepointinfo::CodepointInfo;
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
    if v != 0 {
        disable_screen_checks();
    } else {
        enable_screen_checks();
    }
}

pub type LineIndex = usize;

#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Rect {
    pub x: usize,
    pub y: usize,
    pub width: usize,
    pub height: usize,
}

/// A Screen is composed of Line(s).<br/>
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Screen {
    pub is_off_screen: bool, // Hints
    pub has_eof: bool,       // Hints

    /// the underlying lines storage
    pub line: Vec<Line>,
    /// the index of the line filled with the push method
    pub current_line_index: LineIndex,

    //
    clip: Rect,

    /// maximum number of elements the screen line can hold
    max_width: usize,
    /// maximum number of lines the screen can hold
    max_height: usize,
    /// the number of elements pushed in the screen
    pub push_count: usize,
    // the maximum number of elements the screen can hold
    pub push_capacity: usize,
    /// placeholder to record the offset of the first pushed CodepointInfo (used by View)
    pub first_offset: Option<u64>,
    /// placeholder to record the offset of the last pushed CodepointInfo (used by View)
    pub last_offset: Option<u64>,
    /// placeholder to record the maximum offset of the document (eof)
    pub doc_max_offset: u64,
}

impl Screen {
    pub fn with_dimension(dim: (usize, usize)) -> Screen {
        Screen::new(dim.0, dim.1)
    }

    pub fn dimension(&self) -> (usize, usize) {
        (self.width(), self.height())
    }

    pub fn new(width: usize, height: usize) -> Screen {
        let mut line: Vec<Line> = Vec::new();
        for _ in 0..height {
            line.push(Line::new(width));
        }

        let push_capacity = width * height;
        Screen {
            is_off_screen: false,
            has_eof: false,
            line,
            current_line_index: 0,
            clip: Rect {
                x: 0,
                y: 0,
                width,
                height,
            },
            max_width: width,
            max_height: height,
            push_count: 0,
            push_capacity,
            first_offset: None,
            last_offset: None,
            doc_max_offset: 0,
        }
    }

    pub fn check_invariants(&self) {
        return;

        if self.push_count == 0 {
            return;
        }

        if SCREEN_CHECK_FLAG.load(Ordering::Relaxed) == 0 {
            return;
        }

        // getenv ?
        let mut prev_cpis = vec![];

        if self.last_offset.is_none() {
            return;
        }
        if self.first_offset.is_none() {
            return;
        }

        let last_offset = *self.last_offset.as_ref().clone().unwrap();
        let mut cur_offset = *self.first_offset.as_ref().clone().unwrap();

        if self.has_eof() {
            // last_offset += 1;
        }

        for y in 0..self.line.len() {
            for x in 0..self.line[y].cells.len() {
                let cell = &self.line[y].cells[x];
                let cell_is_used = cell.is_used;

                let cpi = &cell.cpi;

                if cell_is_used && cpi.size > 0 && cpi.metadata == true {
                    dbg_println!(
                        "INVALID PUSH [META] CHECKING cur_offset = {} , CPI {:?}, ",
                        cur_offset,
                        cpi
                    );
                    panic!("");
                }
                if cell_is_used && cpi.size == 0 && cpi.metadata == false {
                    dbg_println!(
                        "INVALID PUSH [NON META] CHECKING cur_offset = {} , CPI {:?}, ",
                        cur_offset,
                        cpi
                    );
                    panic!("");
                }

                if cpi.metadata {
                    continue; // ignore offset + size
                }

                if let Some(offset) = cpi.offset {
                    if cur_offset < offset || cur_offset > last_offset {
                        dbg_println!(
                            "(X({}), Y({})) cur_offset( {} ) >= offset( {} ) < last_offset( {} ) NOT TRUE",
                            x,
                            y,
                            cur_offset,
                            offset,
                            last_offset
                        );
                        dbg_println!("----- BUG screen invariants broken ------- ");
                        dbg_println!("cpi = {:?}", cpi);

                        for prev_cpi in prev_cpis.iter().rev().take(32).rev() {
                            dbg_println!("PREV_CPI = {:?}", prev_cpi);
                        }

                        loop {
                            let wait = std::time::Duration::from_millis(2000);
                            std::thread::sleep(wait);
                        }
                    };

                    prev_cpis.push(cpi.clone());

                    cur_offset += cpi.size as u64;
                    if cpi.metadata == false {
                        // dbg_println!("CUR : UPDATE {:?}", cpi);
                    }
                    if cur_offset >= last_offset {
                        break;
                    }
                } else {
                    assert!(cpi.size == 0);
                    continue;
                    /*
                    dbg_println!(
                        "(X({}), Y({})) NO offset CPI.cp = {:?} CPI.size {}",
                        x,
                        y,
                        cpi.cp,
                        cpi.size
                    );
                    */
                }
            }
        }
    }

    pub fn copy_to(&mut self, x: usize, y: usize, src: &Screen) -> bool {
        if x + src.width() > self.width() || y + src.height() > self.height() {
            return false;
        }

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

    pub fn clip_rect(&self) -> Rect {
        self.clip.clone()
    }

    pub fn width(&self) -> usize {
        self.clip.width - self.clip.x
    }

    pub fn height(&self) -> usize {
        self.clip.height - self.clip.y
    }

    // TODO: return bool
    pub fn set_clipping(&mut self, x: usize, y: usize, width: usize, height: usize) {
        assert!(x < self.max_width);
        assert!(width <= self.max_width);
        assert!(x + width <= self.max_width);

        assert!(y < self.max_height);
        assert!(height <= self.max_height);
        assert!(y + height <= self.max_height);

        for i in y..y + height {
            self.line[i].set_clipping(x, width);
        }

        // store
        self.clip = Rect {
            x,
            y,
            width: x + width,
            height: y + height,
        };

        self.current_line_index = 0; // TODO: save screen state and restore while switching clip
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.line.resize(height, Line::new(width));
        for i in 0..height {
            self.line[i].resize(width);
        }
        self.max_height = height;
        self.max_width = width;
        self.set_clipping(0, 0, width, height);

        self.current_line_index = 0;
        self.push_count = 0;
        self.first_offset = None;
        self.last_offset = None;
        self.doc_max_offset = 0;
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

    pub fn select_next_line_index(&mut self) -> LineIndex {
        assert!(self.current_line_index <= self.height());
        if self.current_line_index == self.height() {
            return self.current_line_index;
        }

        // forced ?
        self.line[self.clip.y + self.current_line_index].read_only = true;
        self.current_line_index += 1; // go to next line
        self.current_line_index
    }

    /// 0-----skip---cur_index----max_height---capacity

    /// append
    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, LineIndex) {
        self.check_invariants();

        if true {
            if cpi.metadata == false && cpi.size == 0 {
                dbg_println!("CPI = {:?}", cpi);
                panic!();
            }
            if cpi.metadata == true && cpi.size != 0 {
                dbg_println!("CPI = {:?}", cpi);
                panic!();
            }

            assert!(self.current_line_index <= self.height());
        }

        if self.current_line_index == self.height() {
            return (false, self.current_line_index);
        }

        // cur line remain cells = self.line[self.clip.y + self.current_line_index].available()
        let unicode_width = UnicodeWidthChar::width(cpi.cp).unwrap_or(1);
        if unicode_width > self.line[self.clip.y + self.current_line_index].available() {
            self.line[self.clip.y + self.current_line_index].read_only = true; // api ?
            self.current_line_index += 1; // go to next line ?

            if self.current_line_index == self.height() {
                return (false, self.current_line_index);
            }
        }

        if self.line[self.clip.y + self.current_line_index].read_only {
            // was marked
            self.current_line_index += 1; // go to next line
            if self.current_line_index == self.height() {
                return (false, self.current_line_index);
            }
        }

        if self.push_count > 0 {
            let cpi_offset = cpi.offset.clone();
            let last_offset = self.last_offset.clone();

            if true {
                match (cpi_offset, last_offset) {
                    (Some(cpi_offset), Some(last_offset)) => {
                        if cpi_offset < last_offset {
                            dbg_println!(
                                "cpi_offset {:?} < last_offset {:?}",
                                cpi_offset,
                                last_offset
                            );

                            // panic!(); allow unsorted offsets ?
                        }
                    }
                    _ => {}
                }
            }
        }

        let line = &mut self.line[self.clip.y + self.current_line_index];
        let (ok, _) = line.push(cpi);
        if ok {
            if self.push_count == 0 {
                self.first_offset = cpi.offset.clone();
            }
            self.last_offset = cpi.offset.clone();
            self.push_count += 1;
        } else {
            // FIXME() handle no space
            if self.line[self.clip.y + self.current_line_index].read_only {
                return self.push(cpi);
            }
        }

        (ok, self.current_line_index)
    }

    pub fn append(&mut self, cpi_vec: &Vec<CodepointInfo>) -> (usize, LineIndex, Option<u64>) {
        for (idx, cpi) in cpi_vec.iter().enumerate() {
            let ret = self.push(*cpi);
            if ret.0 == false {
                // cannot push screen full
                return (idx, ret.1, self.last_offset);
            }
        }

        (cpi_vec.len(), self.current_line_index, self.last_offset)
    }

    pub fn fill_with_cpi_until_eol(&mut self, cpi: CodepointInfo) -> (bool, LineIndex) {
        if self.current_line_index == self.height() {
            return (false, self.current_line_index);
        }

        let remain = {
            let line = &mut self.line[self.clip.y + self.current_line_index];
            let remain = line.available();
            if remain == 0 || line.read_only {
                return (false, self.current_line_index);
            }
            let _count = line.nb_cells;
            remain
        };

        for _ in 0..remain {
            let (ok, li) = self.push(cpi);
            if !ok {
                return (ok, li);
            }
        }

        return (false, self.current_line_index);
    }

    pub fn clear(&mut self) {
        for h in 0..self.max_height {
            self.line[h].clear();
        }
        self.is_off_screen = false;
        self.has_eof = false;

        self.current_line_index = 0;
        self.push_count = 0;
        self.push_capacity = self.max_width * self.max_height;
        self.first_offset = None;
        self.last_offset = None;
        self.doc_max_offset = 0; // TODO: Option<u64>
        self.set_clipping(0, 0, self.max_width, self.max_height);
    }

    pub fn has_eof(&self) -> bool {
        self.has_eof
    }

    pub fn set_has_eof(&mut self) {
        self.has_eof = true;
    }

    pub fn max_width(&self) -> usize {
        self.max_width
    }

    pub fn max_height(&self) -> usize {
        self.max_height
    }

    pub fn get_unclipped_line_mut(&mut self, index: usize) -> Option<&mut Line> {
        if index < self.max_height {
            Some(&mut self.line[index])
        } else {
            None
        }
    }

    pub fn get_line_mut(&mut self, index: usize) -> Option<&mut Line> {
        if index < self.height() {
            Some(&mut self.line[self.clip.y + index])
        } else {
            None
        }
    }

    pub fn get_unclipped_line(&self, index: usize) -> Option<&Line> {
        if index < self.max_height {
            Some(&self.line[index])
        } else {
            None
        }
    }

    pub fn get_line(&self, index: usize) -> Option<&Line> {
        if index < self.height() {
            Some(&self.line[self.clip.y + index])
        } else {
            None
        }
    }

    pub fn get_used_line_mut(&mut self, index: usize) -> Option<&mut Line> {
        if index <= self.current_line_index && self.line[self.clip.y + index].nb_cells > 0 {
            Some(&mut self.line[self.clip.y + index])
        } else {
            None
        }
    }

    pub fn get_used_line_clipped(&mut self, index: usize) -> (Option<&Line>, LineIndex) {
        let index = ::std::cmp::min(index, self.current_line_index);
        (self.get_used_line(index), index)
    }

    // improve this: loop over line push_count > 0
    pub fn get_used_line(&self, index: usize) -> Option<&Line> {
        if index >= self.height() {
            return None;
        }

        if index <= self.current_line_index && self.line[self.clip.y + index].nb_cells > 0 {
            Some(&self.line[self.clip.y + index])
        } else {
            None
        }
    }

    pub fn get_first_used_line(&self) -> Option<&Line> {
        if self.push_count() == 0 {
            None
        } else {
            Some(&self.line[self.clip.y])
        }
    }

    pub fn get_last_used_line(&self) -> Option<&Line> {
        if self.push_count() == 0 {
            return None;
        }

        let mut idx = self.get_last_used_line_index();

        if idx >= self.line.len() {
            dbg_println!("screen = {:?}", self);
            panic!("");
        }

        if self.line[idx].nb_cells == 0 && idx > 1 {
            idx -= 1;
        }

        if self.line[idx].nb_cells == 0 {
            None
        } else {
            Some(&self.line[idx])
        }
    }

    pub fn get_last_used_line_index(&self) -> LineIndex {
        if self.current_line_index == self.height() {
            self.current_line_index - 1
        } else {
            self.current_line_index
        }
    }

    pub fn get_cpinfo(&self, x: usize, y: usize) -> Option<&CodepointInfo> {
        match self.get_line(y) {
            None => None,
            Some(l) => l.get_cpi(x),
        }
    }

    pub fn get_cpinfo_mut(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        match self.get_line_mut(y) {
            None => None,
            Some(l) => l.get_mut_cpi(x),
        }
    }

    pub fn get_used_cpinfo(&self, x: usize, y: usize) -> Option<&CodepointInfo> {
        match self.get_used_line(y) {
            None => None,
            Some(l) => l.get_used_cpi(x),
        }
    }

    pub fn get_used_cpinfo_mut(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        match self.get_used_line_mut(y) {
            None => None,
            Some(l) => l.get_mut_used_cpi(x),
        }
    }

    pub fn get_mut_first_cpinfo(&mut self) -> (Option<&mut CodepointInfo>, usize, usize) {
        match self.get_used_line_mut(0) {
            None => (None, 0, 0),
            Some(l) => (l.get_mut_used_cpi(0), 0, 0),
        }
    }

    pub fn get_last_cpinfo_mut(&mut self) -> (Option<&mut CodepointInfo>, usize, usize) {
        let y = self.current_line_index;
        match self.get_used_line_mut(y) {
            None => (None, 0, 0),
            Some(l) => {
                let x = l.nb_cells;
                (l.get_mut_used_cpi(0), x, y)
            }
        }
    }

    pub fn get_first_cpinfo(&self) -> (Option<&CodepointInfo>, usize, usize) {
        match self.get_used_line(0) {
            None => (None, 0, 0),
            Some(l) => (l.get_used_cpi(0), 0, 0),
        }
    }

    pub fn get_last_cpinfo(&self) -> (Option<&CodepointInfo>, usize, usize) {
        // TODO: check
        let y = if self.current_line_index == self.clip.height {
            self.current_line_index - 1
        } else {
            self.current_line_index
        };

        match self.get_used_line(y) {
            None => (None, 0, 0),
            Some(l) => {
                if l.nb_cells > 0 {
                    let x = l.nb_cells - 1;
                    (l.get_used_cpi(0), x, y)
                } else {
                    (None, 0, 0)
                }
            }
        }
    }

    pub fn get_used_cpinfo_unclipped(
        &mut self,
        x: usize,
        y: usize,
    ) -> (Option<&CodepointInfo>, LineCellIndex, LineIndex) {
        match self.get_used_line(y) {
            None => (None, x, y),
            Some(l) => match l.get_used_cpi(x) {
                Some(optcpi) => (Some(optcpi), x, y),
                None => (None, x, y),
            },
        }
    }

    pub fn at_xy_mut(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        self.get_cpinfo_mut(x, y)
    }

    pub fn at_xy(&self, x: usize, y: usize) -> Option<&CodepointInfo> {
        self.get_cpinfo(x, y)
    }

    pub fn find_cpi_by_offset(&self, offset: u64) -> (Option<&CodepointInfo>, usize, usize) {
        if self.first_offset.is_none() || self.last_offset.is_none() {
            return (None, 0, 0);
        }

        let first_offset = self.first_offset.unwrap();
        let last_offset = self.last_offset.unwrap();

        if offset < first_offset || offset > last_offset {
            return (None, 0, 0);
        }

        let mut max = self.current_line_index;

        let mut min = 0;
        while min <= max {
            let idx = min + (max - min) / 2;
            let l = self.get_line(idx).unwrap();
            let f_cpi = l.get_first_cpi().unwrap();
            let l_cpi = l.get_last_cpi().unwrap();

            if f_cpi.offset.is_none() || l_cpi.offset.is_none() {
                panic!("");
            }

            let first_offset = f_cpi.offset.unwrap();
            let last_offset = l_cpi.offset.unwrap();

            if offset >= first_offset && offset <= last_offset {
                // TODO: handle line.skip / used
                for x in 0..l.width() {
                    let cpi = l.get_cpi(x).unwrap();
                    if cpi.metadata {
                        // continue;
                    }

                    if let Some(cpi_offset) = cpi.offset {
                        if offset == cpi_offset {
                            return (Some(cpi), x, idx);
                        }

                        if cpi_offset < offset && offset < cpi_offset + cpi.size as u64 {
                            return (Some(cpi), x, idx);
                        }
                    }
                }

                dbg_println!("DUMP wrong line: we are looking for offset {}", offset);
                for x in 0..l.width() {
                    let cpi = l.get_cpi(x).unwrap();
                    dbg_println!("cpi[{}] = {:?}", x, cpi);
                }

                panic!(""); // line is wrong
            } else if offset > last_offset {
                min = idx + 1;
            } else {
                max = idx - 1;
            }
        }

        dbg_println!("SELF {:?}", self);

        panic!(
            "cannot file offset {} between {} {}",
            offset, first_offset, last_offset
        );
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

        let (cpi, _, _) = self.find_cpi_by_offset(offset);
        match cpi {
            Some(_) => true,
            _ => false,
        }
    }
}

pub fn screen_apply<F: FnMut(usize, usize, &mut CodepointInfo) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            for c in 0..line.nb_cells {
                if let Some(cpi) = line.get_mut_cpi(c) {
                    if on_cpi(c, l, cpi) == false {
                        return;
                    }
                }
            }
        }
    }
}

pub fn screen_apply_all<F: FnMut(usize, usize, &mut CodepointInfo) -> bool>(
    screen: &mut Screen,
    mut on_cpi: F,
) {
    for l in 0..screen.height() {
        if let Some(line) = screen.get_line_mut(l) {
            for c in 0..line.width() {
                if let Some(cpi) = line.get_mut_cpi(c) {
                    if on_cpi(c, l, cpi) == false {
                        return;
                    }
                }
            }
        }
    }
}

fn _print_clipped_line(screen: &mut Screen, color: (u8, u8, u8), s: &str) {
    let mut push_count = 0;
    for c in s.chars().take(screen.width()) {
        let mut cpi = CodepointInfo::new();
        cpi.metadata = true;
        cpi.style.is_selected = true;
        cpi.cp = c;
        cpi.displayed_cp = c;
        cpi.style.color = color;
        screen.push(cpi);
        push_count += 1;
    }

    // fill line
    for _ in push_count..screen.width() {
        let mut cpi = CodepointInfo::new();
        cpi.metadata = true;
        cpi.style.is_selected = true;

        cpi.cp = ' ';
        cpi.displayed_cp = ' ';
        cpi.style.color = color;
        screen.push(cpi);
    }
}

#[test]
fn test_screen() {
    let mut scr = Screen::new(640, 480);
    assert_eq!(640, scr.width());
    assert_eq!(480, scr.height());
    assert_eq!(scr.height(), scr.line.len());
    assert_eq!(scr.width(), scr.line[0].cells.len());

    scr.resize(800, 600);
    assert_eq!(800, scr.width());
    assert_eq!(600, scr.height());
    assert_eq!(scr.height(), scr.line.len());
    assert_eq!(scr.width(), scr.line[0].cells.len());

    scr.resize(1024, 768);
    assert_eq!(1024, scr.width());
    assert_eq!(768, scr.height());
    assert_eq!(scr.height(), scr.line.len());
    assert_eq!(scr.width(), scr.line[0].cells.len());

    scr.resize(640, 480);
    assert_eq!(640, scr.width());
    assert_eq!(480, scr.height());
    assert_eq!(scr.height(), scr.line.len());
    assert_eq!(scr.width(), scr.line[0].cells.len());
}
