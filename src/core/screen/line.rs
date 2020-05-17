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

use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::core::codepointinfo::CodepointInfo;

pub type LineCellIndex = usize;

/// A LineCell encapsulates code point information (CodepoinInfo).<br/>
/// The displayed Lines are composed of LineCell
#[derive(Hash, Default, Debug, Clone, Eq, PartialEq)]
pub struct LineCell {
    pub cpi: CodepointInfo,
    pub is_used: bool,
}

impl LineCell {
    pub fn new() -> Self {
        LineCell {
            cpi: CodepointInfo::new(),
            is_used: false,
        }
    }
}

/// A Line is an array of LineCell and some metadata.<br/>
/// A Screen is composed of Line(s)
#[derive(Default, Debug, Clone, Eq, PartialEq)]
pub struct Line {
    pub cells: Vec<LineCell>,
    pub nb_cells: usize,
    pub max_width: usize,
    pub start_index: usize,
    width: usize,
    pub read_only: bool,
    pub metadata: bool,
    hash_cache: u64,
    hash_unclipped_cache: u64,
}

impl Line {
    pub fn new(width: LineCellIndex) -> Self {
        assert_eq!(width > 0, true);

        let mut cells = Vec::with_capacity(width);
        for _ in 0..width {
            cells.push(LineCell::new());
        }

        Line {
            cells,
            nb_cells: 0,
            max_width: width,
            start_index: 0,
            width,
            read_only: false,
            metadata: false,
            hash_cache: 0,
            hash_unclipped_cache: 0,
        }
    }

    pub fn hash(&mut self) -> u64 {
        if self.hash_cache != 0 {
            return self.hash_cache;
        }

        let mut s = DefaultHasher::new();

        for i in 0..self.width() {
            self.cells[self.start_index + i].hash(&mut s);
        }

        self.hash_cache = s.finish();
        self.hash_cache
    }

    pub fn hash_unclipped(&mut self) -> u64 {
        if self.hash_unclipped_cache != 0 {
            return self.hash_unclipped_cache;
        }

        let mut s = DefaultHasher::new();

        for i in 0..self.max_width {
            self.cells[i].hash(&mut s);
        }
        self.hash_unclipped_cache = s.finish();
        self.hash_unclipped_cache
    }

    pub fn width(&self) -> usize {
        self.width - self.start_index
    }

    pub fn max_width(&self) -> usize {
        self.max_width
    }

    pub fn resize(&mut self, width: LineCellIndex) {
        self.cells.resize(width, LineCell::new());
        self.nb_cells = 0;
        self.max_width = width;
        self.start_index = 0;
        self.width = width;
        self.read_only = false;
        self.metadata = false;
        self.hash_cache = 0;
    }

    /// returns (start_index, width) tupple
    /// [ 0 <= start_index < width <= screen.max_width() ]
    pub fn clipping(&mut self) -> (usize, usize) {
        (self.start_index, self.width)
    }

    /// [ 0 <= start_index < width <= screen.max_width() ]
    pub fn set_clipping(&mut self, start_index: usize, width: usize) {
        assert!(start_index < self.max_width);
        assert!(width <= self.max_width);
        assert!(start_index + width <= self.max_width);

        self.start_index = start_index;
        self.width = start_index + width;
        self.hash_cache = 0;
        self.nb_cells = 0;
        self.read_only = false;
    }

    pub fn clear_clip_width(&mut self) {
        self.set_clipping(0, self.max_width);
    }

    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, LineCellIndex) {
        if self.nb_cells < self.width() && !self.read_only {
            self.cells[self.start_index + self.nb_cells].cpi = cpi;
            self.cells[self.start_index + self.nb_cells].is_used = true;

            self.nb_cells += 1;

            if self.nb_cells == self.width {
                self.read_only = true;
            }

            (true, self.nb_cells)
        } else {
            self.read_only = true;
            (false, self.nb_cells)
        }
    }

    pub fn clear(&mut self) {
        for i in 0..self.width() {
            self.cells[self.start_index + i] = LineCell::new();
        }

        self.hash_cache = 0;
        self.nb_cells = 0;
        self.read_only = false;
        self.metadata = false;
        self.hash_cache = 0;
    }

    pub fn get_first_cpi(&self) -> Option<&CodepointInfo> {
        if self.start_index < self.width {
            Some(&self.cells[self.start_index].cpi)
        } else {
            // internal error
            None
        }
    }

    pub fn get_last_cpi(&self) -> Option<&CodepointInfo> {
        if self.nb_cells > 0 {
            Some(&self.cells[self.start_index + self.nb_cells - 1].cpi)
        } else {
            None
        }
    }


    pub fn get_unclipped_cpi(&self, index: LineCellIndex) -> Option<&CodepointInfo> {
        if index < self.max_width() {
            Some(&self.cells[index].cpi)
        } else {
            None
        }
    }

    pub fn get_mut_unclipped_cpi(&mut self, index: LineCellIndex) -> Option<&mut CodepointInfo> {
        if index < self.max_width() {
            Some(&mut self.cells[index].cpi)
        } else {
            None
        }
    }

    pub fn get_cpi(&self, index: LineCellIndex) -> Option<&CodepointInfo> {
        if index < self.width() {
            Some(&self.cells[self.start_index + index].cpi)
        } else {
            None
        }
    }

    pub fn get_mut_cpi(&mut self, index: LineCellIndex) -> Option<&mut CodepointInfo> {
        if index < self.width() {
            Some(&mut self.cells[self.start_index + index].cpi)
        } else {
            None
        }
    }

    pub fn at(&mut self, index: LineCellIndex) -> Option<&mut CodepointInfo> {
        self.get_mut_cpi(index)
    }

    pub fn get_used_cpi(&self, index: LineCellIndex) -> Option<&CodepointInfo> {
        if index < self.nb_cells {
            self.get_cpi(index)
        } else {
            None
        }
    }

    pub fn get_mut_used_cpi(&mut self, index: LineCellIndex) -> Option<&mut CodepointInfo> {
        if index < self.nb_cells {
            self.get_mut_cpi(index)
        } else {
            None
        }
    }

    pub fn get_used_cpi_clipped(
        &self,
        index: LineCellIndex,
    ) -> (Option<&CodepointInfo>, LineCellIndex) {
        if self.nb_cells == 0 {
            return (None, 0);
        }

        let index = ::std::cmp::min(index, self.nb_cells - 1);
        (self.get_used_cpi(index), index)
    }
}
