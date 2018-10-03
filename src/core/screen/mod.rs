pub mod line;

use core::codepointinfo::CodepointInfo;

use self::line::Line;
use self::line::LineCellIndex;

pub type LineIndex = usize;

/// A Screen is composed of Line(s).<br/>
#[derive(Debug, Clone, Eq, PartialEq)]
pub struct Screen {
    /// the underlying lines storage
    pub line: Vec<Line>,
    /// the index of the line filled with the puhs method
    pub current_line_index: LineCellIndex,
    /// maximum number of elements the screen line can hold
    pub width: usize,
    /// maximum number of lines the screen can hold
    pub height: usize,
    /// the number of elements pushed in the screen
    pub nb_push: usize,
    /// placeholder to record the offset of the first pushed CodepointInfo (used by View)
    pub first_offset: u64,
    /// placeholder to record the offset of the last pushed CodepointInfo (used by View)
    pub last_offset: u64,
    /// placeholder to record the maximum offset of the document (eof)
    pub doc_max_offset: u64,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Screen {
        let mut line: Vec<Line> = Vec::new();
        for _ in 0..height {
            line.push(Line::new(width));
        }

        Screen {
            line,
            current_line_index: 0,
            width,
            height,
            nb_push: 0,
            first_offset: 0,
            last_offset: 0,
            doc_max_offset: 0,
        }
    }

    pub fn resize(&mut self, width: usize, height: usize) {
        self.line.resize(height, Line::new(width));
        for i in 0..height {
            self.line[i].resize(width);
        }
        self.width = width;
        self.height = height;
        self.current_line_index = 0;
        self.nb_push = 0;
        self.first_offset = 0;
        self.last_offset = 0;
        self.doc_max_offset = 0;
    }

    /// append
    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {
        if self.current_line_index == self.height {
            return (false, self.current_line_index);
        }

        if self.line[self.current_line_index].read_only {
            self.current_line_index += 1;
        }

        if self.current_line_index == self.height {
            return (false, self.current_line_index);
        }

        let cp = cpi.cp;
        let line = &mut self.line[self.current_line_index];
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
        for h in 0..self.height {
            self.line[h].clear();
        }
        self.current_line_index = 0;
        self.nb_push = 0;
        self.first_offset = 0;
        self.last_offset = 0;
        self.doc_max_offset = 0;
    }

    pub fn get_mut_line(&mut self, index: usize) -> Option<&mut Line> {
        if index < self.height {
            Some(&mut self.line[index])
        } else {
            None
        }
    }

    pub fn get_line(&self, index: usize) -> Option<&Line> {
        if index < self.height {
            Some(&self.line[index])
        } else {
            None
        }
    }

    pub fn get_mut_used_line(&mut self, index: usize) -> Option<&mut Line> {
        if index < self.current_line_index {
            Some(&mut self.line[index])
        } else {
            None
        }
    }

    pub fn get_used_line_clipped(&mut self, index: usize) -> (Option<&Line>, LineIndex) {
        let index = ::std::cmp::min(index, self.current_line_index);
        (self.get_used_line(index), index)
    }

    pub fn get_used_line(&self, index: usize) -> Option<&Line> {
        if index < self.current_line_index {
            Some(&self.line[index])
        } else {
            None
        }
    }

    /// there must be 2 line a least
    pub fn get_first_used_line(&self) -> Option<&Line> {
        if 0 < self.current_line_index {
            Some(&self.line[0])
        } else {
            None
        }
    }

    /// there must be 2 line a least
    pub fn get_last_used_line(&self) -> Option<&Line> {
        if self.current_line_index > 0 {
            Some(&self.line[self.current_line_index - 1])
        } else {
            None
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
        let y = self.current_line_index;
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

    pub fn get_used_cpinfo_clipped(
        &mut self,
        x: usize,
        y: usize,
    ) -> (Option<&CodepointInfo>, LineCellIndex, LineIndex) {
        match self.get_used_line_clipped(y) {
            (None, li) => (None, x, li),
            (Some(l), li) => match l.get_used_cpi_clipped(x) {
                (optcpi, lci) => (optcpi, lci, li),
            },
        }
    }

    pub fn find_cpi_by_offset(&self, offset: u64) -> (Option<&CodepointInfo>, usize, usize) {
        // TODO: use dichotomic search

        if offset < self.first_offset || offset > self.last_offset {
            return (None, 0, 0);
        }

        for y in 0..self.height {
            let l = self.get_line(y).unwrap();
            if l.nb_cells == 0 {
                continue;
            }

            for x in 0..l.width {
                let cpi = l.get_cpi(x).unwrap();
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
    assert_eq!(640, scr.width);
    assert_eq!(480, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].cells.len());

    scr.resize(800, 600);
    assert_eq!(800, scr.width);
    assert_eq!(600, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].cells.len());

    scr.resize(1024, 768);
    assert_eq!(1024, scr.width);
    assert_eq!(768, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].cells.len());

    scr.resize(640, 480);
    assert_eq!(640, scr.width);
    assert_eq!(480, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].cells.len());
}
