use core::codepointinfo::CodepointInfo;




// a line is composed of codepoints
#[derive(Debug, Clone)]
pub struct Line {
    pub chars: Vec<CodepointInfo>,
    pub nb_chars: usize,
    pub width: usize,
    pub read_only: bool,
}

impl Line {
    pub fn new(width: usize) -> Line {

        assert_eq!(width > 0, true);

        let mut chars = Vec::with_capacity(width);
        for _ in 0..width {
            chars.push(CodepointInfo::new());
        }

        Line {
            chars,
            nb_chars: 0,
            width,
            read_only: false,
        }
    }

    pub fn resize(&mut self, width: usize) {
        self.chars.resize(width, CodepointInfo::new());
        self.nb_chars = 0;
        self.width = width;
        self.read_only = false;
    }

    pub fn push(&mut self, cpi: CodepointInfo) -> (bool, usize) {

        if self.nb_chars < self.width && self.read_only == false {
            self.chars[self.nb_chars] = cpi;
            self.nb_chars += 1;

            if self.nb_chars == self.width {
                self.read_only = true;
            }

            (true, self.nb_chars)
        } else {
            self.read_only = true;
            (false, self.nb_chars)
        }



    }

    pub fn clear(&mut self) {
        for w in 0..self.width {
            self.chars[w] = CodepointInfo::new();
        }
        self.nb_chars = 0;
        self.read_only = false;
    }

    pub fn get_cpi(&self, index: usize) -> Option<&CodepointInfo> {
        if index < self.width {
            Some(&self.chars[index])
        } else {
            None
        }
    }

    pub fn get_first_cpi(&self) -> Option<&CodepointInfo> {
        if 0 < self.nb_chars {
            Some(&self.chars[0])
        } else {
            None
        }
    }

    pub fn get_last_cpi(&self) -> Option<&CodepointInfo> {
        if self.nb_chars > 0 {
            Some(&self.chars[self.nb_chars - 1])
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
        if index < self.nb_chars {
            Some(&self.chars[index])
        } else {
            None
        }
    }

    pub fn get_mut_used_cpi(&mut self, index: usize) -> Option<&mut CodepointInfo> {
        if index < self.nb_chars {
            Some(&mut self.chars[index])
        } else {
            None
        }
    }
}
