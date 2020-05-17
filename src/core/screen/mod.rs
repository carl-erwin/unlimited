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

pub mod line;
use crate::core::codepointinfo::CodepointInfo;
use std::time::Duration;

use self::line::Line;
use self::line::LineCellIndex;

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
    pub nb_push: usize,
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

        Screen {
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

            nb_push: 0,
            first_offset: 0,
            last_offset: 0,
            doc_max_offset: 0,
            time_to_build: Duration::new(0, 0),
            input_size: 0,
        }
    }

    pub fn check_invariants(&self) {
        if self.nb_push == 0 {
            return;
        }

        if self.first_offset == self.last_offset && self.first_offset == 0 {
            // forget to clear screen ?
            panic!("");
        }
    }

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
        self.nb_push = 0;
        self.first_offset = 0;
        self.last_offset = 0;
        self.doc_max_offset = 0;
        self.input_size = 0;
    }

    /// 0-----skip---cur_index----max_height---capacity

    /// append
    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {
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
            self.nb_push += 1;
            if cp == '\n' || cp == '\r' {
                line.read_only = true;
            }
        }
        (ok, self.current_line_index)
    }

    pub fn clear(&mut self) {
        for h in 0..self.max_height {
            self.line[h].clear();
        }
        self.current_line_index = 0;
        self.nb_push = 0;
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

    // improve this: loop over line nb_push > 0
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

    /// there must be 2 lines a least
    pub fn get_first_used_line(&self) -> Option<&Line> {
        if 0 < self.current_line_index {
            Some(&self.line[self.clip.y])
        } else {
            None
        }
    }

    /// there must be 2 line a least
    pub fn get_last_used_line(&self) -> Option<&Line> {
        if self.current_line_index > 0 {
            Some(&self.line[self.clip.y + self.current_line_index - 1])
        } else {
            None
        }
    }

    pub fn get_last_used_line_index(&self) -> usize {
        self.current_line_index
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
        // TODO: use dichotomic search

        if offset < self.first_offset || offset > self.last_offset {
            return (None, 0, 0);
        }

        for y in 0..self.height() {
            let l = self.get_line(y).unwrap();
            if l.nb_cells == 0 {
                // continue; // TODO clipping ....
            }

            // TODO: handle line.skip
            for x in 0..l.width() {
                let cpi = l.get_cpi(x).unwrap();
                if cpi.metadata {
                    continue;
                }
                if cpi.offset == offset {
                    return (Some(cpi), x, y);
                }
            }
        }
        (None, 0, 0)
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
