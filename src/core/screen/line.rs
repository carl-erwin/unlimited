use core::codepointinfo::CodepointInfo;




// a cell contains codepoint informations
#[derive(Debug, Clone)]
pub struct LineCell {
    pub cpi: CodepointInfo,
    pub is_used: bool,
}

impl LineCell {
    pub fn new() -> LineCell {
        LineCell {
            cpi: CodepointInfo::new(),
            is_used: false,
        }
    }
}

// a line is composed of cells
#[derive(Debug, Clone)]
pub struct Line {
    pub cells: Vec<LineCell>,
    pub nb_cells: usize,
    pub width: usize,
    pub read_only: bool,
}

impl Line {
    pub fn new(width: usize) -> Line {

        assert_eq!(width > 0, true);

        let mut cells = Vec::with_capacity(width);
        for _ in 0..width {
            cells.push(LineCell::new());
        }

        Line {
            cells,
            nb_cells: 0,
            width,
            read_only: false,
        }
    }

    pub fn resize(&mut self, width: usize) {
        self.cells.resize(width, LineCell::new());
        self.nb_cells = 0;
        self.width = width;
        self.read_only = false;
    }

    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {

        if self.nb_cells < self.width && self.read_only == false {
            self.cells[self.nb_cells].cpi = cpi;
            self.cells[self.nb_cells].is_used = true;
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
        for w in 0..self.width {
            self.cells[w] = LineCell::new();
        }
        self.nb_cells = 0;
        self.read_only = false;
    }

    pub fn get_cpi(&self, index: usize) -> Option<&CodepointInfo> {
        if index < self.width {
            Some(&self.cells[index].cpi)
        } else {
            None
        }
    }

    pub fn get_first_cpi(&self) -> Option<&CodepointInfo> {
        if 0 < self.nb_cells {
            Some(&self.cells[0].cpi)
        } else {
            None
        }
    }

    pub fn get_last_cpi(&self) -> Option<&CodepointInfo> {
        if self.nb_cells > 0 {
            Some(&self.cells[self.nb_cells - 1].cpi)
        } else {
            None
        }
    }


    pub fn get_mut_cpi(&mut self, index: usize) -> Option<&mut CodepointInfo> {
        if index < self.width {
            Some(&mut self.cells[index].cpi)
        } else {
            None
        }
    }

    pub fn get_used_cpi(&self, index: usize) -> Option<&CodepointInfo> {
        if index < self.nb_cells {
            Some(&self.cells[index].cpi)
        } else {
            None
        }
    }

    pub fn get_mut_used_cpi(&mut self, index: usize) -> Option<&mut CodepointInfo> {
        if index < self.nb_cells {
            Some(&mut self.cells[index].cpi)
        } else {
            None
        }
    }
}
