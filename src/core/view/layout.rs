use core::codec::text::utf8;
use core::codepointinfo::CodepointInfo;
use core::mark::Mark;
use core::screen::Screen;
use core::view::View;

// TODO: move to core/view/layout.rs
pub fn get_lines_offsets<'a>(
    view: &View<'a>,
    start_offset: u64,
    end_offset: u64,
    screen_width: usize,
    screen_height: usize,
) -> Vec<(u64, u64)> {
    let mut v = Vec::<(u64, u64)>::new();

    let mut m = Mark::new(start_offset);

    let doc = view.document.as_ref().unwrap().borrow_mut();

    let screen_width = ::std::cmp::max(1, screen_width);
    let screen_height = ::std::cmp::max(4, screen_height);

    // get beginning of the line @offset
    m.move_to_beginning_of_line(&doc.buffer, utf8::get_prev_codepoint);

    // and build tmp screens until end_offset if found
    let mut screen = Screen::new(screen_width, screen_height);

    let max_offset = doc.buffer.size as u64;
    let max_size = (screen_width * screen_height * 4) as usize;

    loop {
        // fill screen
        let mut data = vec![];
        doc.buffer.read(m.offset, max_size, &mut data);

        let _ = build_screen_layout(&data, m.offset, max_offset, &mut screen);

        // push lines offsets
        // FIXME: find a better way to iterate over the used lines
        for i in 0..screen.current_line_index {
            if !v.is_empty() && i == 0 {
                // do not push line range twice
                continue;
            }

            let s = screen.line[i].get_first_cpi().unwrap().offset;
            let e = screen.line[i].get_last_cpi().unwrap().offset;

            v.push((s, e));

            if s >= end_offset || e == max_offset {
                return v;
            }
        }

        // eof reached ?
        // FIXME: the api is not yet READY
        // we must find a way to cover all filled lines
        if screen.current_line_index < screen.height {
            let s = screen.line[screen.current_line_index]
                .get_first_cpi()
                .unwrap()
                .offset;

            let e = screen.line[screen.current_line_index]
                .get_last_cpi()
                .unwrap()
                .offset;
            v.push((s, e));
            return v;
        }

        // TODO: activate only in debug builds
        if 0 == 1 {
            match screen.find_cpi_by_offset(m.offset) {
                (Some(cpi), x, y) => {
                    assert_eq!(x, 0);
                    assert_eq!(y, 0);
                    assert_eq!(cpi.offset, m.offset);
                }
                _ => panic!("implementation error"),
            }
        }

        if let Some(l) = screen.get_last_used_line() {
            if let Some(cpi) = l.get_first_cpi() {
                m.offset = cpi.offset; // update next screen start
            }
        }

        screen.clear(); // prepare next screen
    }
}

//////////////////////////////////
// This function will run the configured filters
// until the screen is full or eof is reached
// the filters will be configured per view to allow multiple interpretation of the same document
// data will be replaced by a "FileMMap"
pub fn build_screen_layout(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    screen: &mut Screen,
) -> u64 {
    let max_cpi = screen.width * screen.height;

    // utf8
    let (vec, _) = decode_slice_to_vec(data, base_offset, max_offset, max_cpi);

    // hexa
    //let (vec, _) = raw_slice_to_hex_vec(data, base_offset, max_offset, max_cpi);

    screen.first_offset = base_offset;
    let mut last_pushed_offset = base_offset;
    let mut prev_cp = ' ';
    for cpi in &vec {
        let (ok, _) = match (prev_cp, cpi.cp) {
            // TODO: handle \r\n
            /*
                ('\r', '\n') => {
                    prev_cp = ' ';
                    (true, 0 as usize)
                }
            */
            (_, '\t') => {
                prev_cp = cpi.cp;
                let mut filtered_cp = *cpi;
                filtered_cp.displayed_cp = ' ';

                let mut last = (false, 0);
                for _ in 0..8 {
                    last = screen.push(filtered_cp);
                }
                last
            }

            _ => {
                prev_cp = cpi.cp;
                screen.push(*cpi)
            }
        };
        if !ok {
            break;
        }
        last_pushed_offset = cpi.offset;
    }

    screen.doc_max_offset = max_offset;
    screen.last_offset = last_pushed_offset;
    last_pushed_offset
}

fn decode_slice_to_vec(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    max_cpi: usize,
) -> (Vec<CodepointInfo>, u64) {
    let mut vec = Vec::with_capacity(max_cpi);

    let mut off: u64 = 0;
    let last_off = data.len() as u64;

    while off != last_off {
        let (cp, _, size) = utf8::get_codepoint(data, off);
        vec.push(filter_codepoint(cp, base_offset + off));
        off += size as u64;
        if vec.len() == max_cpi {
            break;
        }
    }

    // eof handling
    if base_offset + last_off == max_offset {
        vec.push(CodepointInfo {
            cp: ' ',
            displayed_cp: '$',
            offset: base_offset + last_off,
            is_selected: true,
        });
    }

    (vec, base_offset + off)
}

//
fn _raw_slice_to_hex_vec(
    data: &[u8],
    base_offset: u64,
    max_offset: u64,
    max_cpi: usize,
) -> (Vec<CodepointInfo>, u64) {
    let mut vec = Vec::with_capacity(max_cpi);

    let mut off: u64 = base_offset;
    let last_off = data.len() as u64;

    let hexchars: [char; 16] = [
        '0', '1', '2', '3', '4', '5', '6', '7', '8', '9', 'a', 'b', 'c', 'd', 'e', 'f',
    ];

    while off < last_off {
        let mut width = 0;
        for i in 0..16 {
            if off + i >= last_off {
                break;
            }

            let hi: usize = (data[(off + i) as usize] >> 4) as usize;
            let low: usize = (data[(off + i) as usize] & 0x0f) as usize;

            let cp = hexchars[hi];
            vec.push(filter_codepoint(cp, off + i));
            let cp = hexchars[low];
            vec.push(filter_codepoint(cp, off + i));
            vec.push(filter_codepoint(' ', off + i));

            if vec.len() == max_cpi {
                break;
            }
            width += 1;
        }

        if 0 == 1 {
            vec.push(filter_codepoint('|', off + width));
            vec.push(filter_codepoint(' ', off + width));

            for i in 0..16 {
                if off + i >= last_off {
                    break;
                }

                let c: char = data[(off + i) as usize] as char;
                vec.push(filter_codepoint(c, off + i));
                if vec.len() == max_cpi {
                    break;
                }
            }
        }

        vec.push(filter_codepoint('\n', off));
        off += width;
    }

    // eof handling
    if last_off == max_offset {
        vec.push(CodepointInfo {
            cp: ' ',
            displayed_cp: '$',
            offset: last_off,
            is_selected: true,
        });
    }

    (vec, off)
}

// TODO return array of CodePointInfo  0x7f -> <ESC>
pub fn filter_codepoint(c: char, offset: u64) -> CodepointInfo {
    let displayed_cp: char = match c {
        '\r' | '\n' | '\t' => ' ',

        _ if c < ' ' => '�',

        _ if c == 0x7f as char => '�',

        _ => c,
    };

    CodepointInfo {
        cp: c,
        displayed_cp,
        offset,
        is_selected: false,
    }
}
