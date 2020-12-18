// Copyright (c) Carl-Erwin Griffith

pub mod line;

use std::time::Duration;



use self::line::Line;
use self::line::LineCellIndex;
use crate::core::codepointinfo::CodepointInfo;

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
    pub first_offset: u64,
    /// placeholder to record the offset of the last pushed CodepointInfo (used by View)
    pub last_offset: u64,
    /// placeholder to record the maximum offset of the document (eof)
    pub doc_max_offset: u64,
    /// time spent to generate the screen content
    pub time_to_build: Duration,

    pub input_size: usize,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Screen {
        assert!(width > 0);
        assert!(height > 0);

        let mut line: Vec<Line> = Vec::new();
        for _ in 0..height {
            line.push(Line::new(width));
        }

        let push_capacity = width * height;
        Screen {
            is_off_screen: false,
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
            first_offset: 0,
            last_offset: 0,
            doc_max_offset: 0,
            time_to_build: Duration::new(0, 0),
            input_size: 0,
        }
    }

    pub fn check_invariants(&self) {}

    pub fn copy_to(&mut self, x: usize, y: usize, src: &Screen) -> bool {
        if x + src.width() > self.width() || y + src.height() > self.height() {
            return false;
        }

        for src_y in 0..src.height() {
            for src_x in 0..src.width() {
                if let Some(cpi_src) = src.get_cpinfo(src_x, src_y) {
                    if let Some(cpi_dst) = self.get_mut_cpinfo(x + src_x, y + src_y) {
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
        self.first_offset = 0;
        self.last_offset = 0;
        self.doc_max_offset = 0;
        self.input_size = 0;
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

    /// 0-----skip---cur_index----max_height---capacity

    /// append
    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, LineIndex) {
        if self.current_line_index == self.height() {
            return (false, self.current_line_index);
        }

        if self.line[self.clip.y + self.current_line_index].read_only {
            self.current_line_index += 1;
        }

        if self.current_line_index == self.height() {
            return (false, self.current_line_index);
        }

        let cp = cpi.cp;
        let line = &mut self.line[self.clip.y + self.current_line_index];
        let (ok, _) = line.push(cpi);

        if ok {
            self.last_offset = cpi.offset;
            self.push_count += 1;
            if cp == '\n' || cp == '\r' {
                // dbg_println!("detected enf of line = line[{}] available is {}", self.current_line_index, line.available());
                // dbg_println!("detected enf of line = line[{}] capacity is {}", self.current_line_index, line.capacity());
                // dbg_println!("detected enf of line = push capacity is {}", self.push_capacity);

                line.read_only = true;
                // substract skipped columns
                if self.push_capacity >= line.available() {
                    self.push_capacity -= line.available();
                } else {
                    self.push_capacity = 0;
                }
            }
        }
        (ok, self.current_line_index)
    }

    pub fn append(&mut self, cpi_vec: &Vec<CodepointInfo>) -> (usize, LineIndex, u64) {
        for (idx, cpi) in cpi_vec.iter().enumerate() {
            let ret = self.push(*cpi);
            if ret.0 == false {
                return (idx, ret.1, self.last_offset);
            }
        }
        (cpi_vec.len(), self.current_line_index, self.last_offset)
    }

    pub fn clear(&mut self) {
        for h in 0..self.max_height {
            self.line[h].clear();
        }
        self.current_line_index = 0;
        self.push_count = 0;
        self.push_capacity = self.max_width * self.max_height;
        self.first_offset = 0;
        self.last_offset = 0;
        self.doc_max_offset = 0;
        self.input_size = 0;
        self.set_clipping(0, 0, self.max_width, self.max_height);
    }

    pub fn max_width(&self) -> usize {
        self.max_width
    }

    pub fn max_height(&self) -> usize {
        self.max_height
    }

    pub fn get_mut_unclipped_line(&mut self, index: usize) -> Option<&mut Line> {
        if index < self.max_height {
            Some(&mut self.line[index])
        } else {
            None
        }
    }

    pub fn get_mut_line(&mut self, index: usize) -> Option<&mut Line> {
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

    pub fn get_mut_used_line(&mut self, index: usize) -> Option<&mut Line> {
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

        if self.current_line_index == self.height() {
            Some(&self.line[self.clip.y + self.current_line_index - 1])
        } else {
            Some(&self.line[self.clip.y + self.current_line_index])
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

    pub fn get_mut_cpinfo(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        match self.get_mut_line(y) {
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

    pub fn get_mut_used_cpinfo(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        match self.get_mut_used_line(y) {
            None => None,
            Some(l) => l.get_mut_used_cpi(x),
        }
    }

    pub fn get_mut_first_cpinfo(&mut self) -> (Option<&mut CodepointInfo>, usize, usize) {
        match self.get_mut_used_line(0) {
            None => (None, 0, 0),
            Some(l) => (l.get_mut_used_cpi(0), 0, 0),
        }
    }

    pub fn get_mut_last_cpinfo(&mut self) -> (Option<&mut CodepointInfo>, usize, usize) {
        let y = self.current_line_index;
        match self.get_mut_used_line(y) {
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

    pub fn at_xy(&mut self, x: usize, y: usize) -> Option<&mut CodepointInfo> {
        self.get_mut_cpinfo(x, y)
    }

    pub fn at_yx(&mut self, y: usize, x: usize) -> Option<&mut CodepointInfo> {
        self.at_xy(x, y)
    }

    pub fn find_cpi_by_offset(&self, offset: u64) -> (Option<&CodepointInfo>, usize, usize) {
        if offset < self.first_offset || offset > self.last_offset {
            return (None, 0, 0);
        }

        let mut max = self.current_line_index;

        let mut min = 0;
        while min <= max {
            let idx = min + (max - min) / 2;
            let l = self.get_line(idx).unwrap();
            let f_cpi = l.get_first_cpi().unwrap();
            let l_cpi = l.get_last_cpi().unwrap();
            if offset >= f_cpi.offset && offset <= l_cpi.offset {
                // TODO: handle line.skip / used
                for x in 0..l.width() {
                    let cpi = l.get_cpi(x).unwrap();
                    if cpi.metadata {
                        continue;
                    }
                    if cpi.offset == offset {
                        return (Some(cpi), x, idx);
                    }
                }

                panic!(""); // TODO: handle meta data
            } else if offset > l_cpi.offset {
                min = idx + 1;
            } else {
                max = idx - 1;
            }
        }

        panic!("");
    }

    pub fn contains_offset(&self, offset: u64) -> bool {
        if offset < self.first_offset || offset > self.last_offset {
            return false;
        }

        let (cpi, _, _) = self.find_cpi_by_offset(offset);
        match cpi {
            Some(_) => true,
            _ => false,
        }
    }
}

fn _print_clipped_line(screen: &mut Screen, color: (u8, u8, u8), s: &str) {
    let mut push_count = 0;
    for c in s.chars().take(screen.width()) {
        let mut cpi = CodepointInfo::new();
        cpi.metadata = true;
        cpi.is_selected = true;
        cpi.cp = c;
        cpi.displayed_cp = c;
        cpi.color = color;
        screen.push(cpi);
        push_count += 1;
    }

    // fill line
    for _ in push_count..screen.width() {
        let mut cpi = CodepointInfo::new();
        cpi.metadata = true;
        cpi.is_selected = true;

        cpi.cp = ' ';
        cpi.displayed_cp = ' ';
        cpi.color = color;
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
