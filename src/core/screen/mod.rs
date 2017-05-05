pub mod line;

use core::codepointinfo::CodepointInfo;

use self::line::Line;



#[derive(Debug, Clone)]
struct Dimension {
    l: usize,
    c: usize,
    w: usize,
    h: usize,
}

// the screen is composed of lines
#[derive(Debug, Clone)]
pub struct Screen {
    pub line: Vec<Line>,
    pub used: usize, // number of used line
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

    pub fn resize(&mut self, width: usize, height: usize) {

        self.line.resize(height, Line::new(width));
        for i in 0..height {
            self.line[i].chars.resize(width, CodepointInfo::new());
            self.line[i].width = width;
            self.line[i].used = 0;
        }
        self.width = width;
        self.height = height;
        self.used = 0;
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

    pub fn get_mut_used_line(&mut self, index: usize) -> Option<&mut Line> {
        if index < self.used {
            Some(&mut self.line[index])
        } else {
            None
        }
    }

    pub fn get_used_line(&self, index: usize) -> Option<&Line> {
        if index < self.used {
            Some(&self.line[index])
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
        let y = self.used;
        match self.get_mut_used_line(y) {
            None => (None, 0, 0),
            Some(l) => {
                let x = l.used;
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
        let y = self.used;
        match self.get_used_line(y) {
            None => (None, 0, 0),
            Some(l) => {
                let x = l.used;
                (l.get_used_cpi(0), x, y)
            }
        }
    }
}



#[test]
fn test_screen() {

    let mut scr = Screen::new(640, 480);
    assert_eq!(640, scr.width);
    assert_eq!(480, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].chars.len());

    scr.resize(800, 600);
    assert_eq!(800, scr.width);
    assert_eq!(600, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].chars.len());


    scr.resize(1024, 768);
    assert_eq!(1024, scr.width);
    assert_eq!(768, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].chars.len());

    scr.resize(640, 480);
    assert_eq!(640, scr.width);
    assert_eq!(480, scr.height);
    assert_eq!(scr.height, scr.line.len());
    assert_eq!(scr.width, scr.line[0].chars.len());
}
