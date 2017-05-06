use core::codepointinfo::CodepointInfo;




// a line is composed of codepoints
#[derive(Debug, Clone)]
pub struct Line {
    pub chars: Vec<CodepointInfo>,
    pub used: usize,
    pub width: usize,
    pub read_only: bool,
}

impl Line {
    pub fn new(columns: usize) -> Line {

        assert_eq!(columns > 0, true);

        let mut chars = Vec::with_capacity(columns);
        for _ in 0..columns {
            chars.push(CodepointInfo::new());
        }

        Line {
            chars,
            used: 0,
            width: columns,
            read_only: false,
        }
    }

    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {

        if self.used < self.width && self.read_only == false {
            self.chars[self.used] = cpi;
            self.used += 1;
            (true, self.used)
        } else {
            self.read_only = true;
            (false, self.used)
        }
    }

    pub fn clear(&mut self) {
        for w in 0..self.width {
            self.chars[w] = CodepointInfo::new();
        }
        self.used = 0;
        self.read_only = false;
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
