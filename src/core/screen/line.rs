use core::codepointinfo::CodepointInfo;




// a line is composed of codepoints
#[derive(Debug, Clone)]
pub struct Line {
    pub chars: Vec<CodepointInfo>,
    pub used: usize,
    pub width: usize,
}

impl Line {
    pub fn new(columns: usize) -> Line {

        let mut chars = Vec::with_capacity(columns);
        for _ in 0..columns {
            chars.push(CodepointInfo::new());
        }

        Line {
            chars,
            used: 0,
            width: columns,
        }
    }

    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {

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
            self.chars[w] = CodepointInfo::new();
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

    pub fn get_used_cpi(&self, index: usize) -> Option<&CodepointInfo> {
        if index < self.used {
            Some(&self.chars[index])
        } else {
            None
        }
    }

    pub fn get_mut_used_cpi(&mut self, index: usize) -> Option<&mut CodepointInfo> {
        if index < self.used {
            Some(&mut self.chars[index])
        } else {
            None
        }
    }
}
