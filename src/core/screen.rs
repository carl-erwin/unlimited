use core::codepointinfo::CodepointInfo;


// the screen is composed of lines
#[derive(Debug)]
pub struct Screen {
    pub line: Vec<Line>,
    pub used: usize,
    pub width: usize,
    pub height: usize,
}

impl Screen {
    pub fn new(width: usize, height: usize) -> Screen {
        let mut line: Vec<Line> = Vec::new();
        for _ in 0..height {
            line.push(Line::new(width));
        }

        Screen {
            line: line,
            used: 0,
            width: width,
            height: height,
        }
    }

    /// append
    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {
        if self.used == self.height {
            return (false, self.used);
        }
        if self.line[self.used].used >= self.line[self.used].width {
            self.used += 1;
        }
        if self.used >= self.height {
            return (false, self.used);
        }

        let cp = cpi.cp;
        let line = &mut self.line[self.used];
        let (ok, _) = line.push(cpi);

        if ok == true {
            if cp == '\n' || cp == '\r' {
                self.used += 1;
            }
        }
        (ok, self.used)
    }

    pub fn clear(&mut self) {
        for h in 0..self.height {
            self.line[h].clear();
        }
        self.used = 0;
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
}

// a line is composed of codepoints
#[derive(Debug)]
pub struct Line {
    pub chars: Vec<CodepointInfo>,
    pub used: usize,
    pub width: usize,
}

impl Line {
    fn new(columns: usize) -> Line {

        let mut chars = Vec::with_capacity(columns);
        for _ in 0..columns {
            chars.push(CodepointInfo {
                           cp: ' ',
                           displayed_cp: ' ',
                           offset: 0,
                           is_selected: false,
                       });
        }

        Line {
            chars: chars,
            used: 0,
            width: columns,
        }
    }

    fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {

        if self.used < self.width {
            self.chars[self.used] = cpi;
            self.used += 1;
            (true, self.used)
        } else {
            (false, self.used)
        }
    }

    pub fn clear(&mut self) {
        for w in 0..self.width {
            self.chars[w].cp = ' ';
            self.chars[w].displayed_cp = ' ';
            self.chars[w].offset = 0;
        }
        self.used = 0;
    }

    pub fn get_cpi(&self, index: usize) -> Option<&CodepointInfo> {
        if index < self.width {
            Some(&self.chars[index])
        } else {
            None
        }
    }

    pub fn get_mut_cpi(&mut self, index: usize) -> Option<&mut CodepointInfo> {
        if index < self.width {
            Some(&mut self.chars[index])
        } else {
            None
        }
    }
}
