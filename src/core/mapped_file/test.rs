extern crate mapped_file;
extern crate rand;

use rand::Rng;
use rand::distributions::{IndependentSample, Range};

use mapped_file::mapped_file::MappedFile;

#[cfg(test)]
use std::rc::Rc;

#[cfg(test)]
use std::cell::RefCell;

#[cfg(test)]
fn wcl(file: &Rc<RefCell<MappedFile>>) {
    println!("------------ WCL {} @ {}", file!(), line!());

    let read_size = 32 * 1024;

    let mut prev_byte = 0;
    let mut nr_lines = 0;
    let mut it = MappedFile::iter_from(&file, 0);
    let mut vec = Vec::with_capacity(read_size);
    loop {
        println!("wc loop");

        println!("try read {} bytes", read_size);

        let n = MappedFile::read(&mut it, read_size, &mut vec);

        println!("read ok");

        for i in &vec {
            match *i as char {
                '\r' => {
                    nr_lines += 1;
                }

                '\n' => {
                    if prev_byte != '\r' as u8 {
                        nr_lines += 1;
                    }
                }

                _ => {}
            }
            prev_byte = *i as u8;
        }

        if n < read_size {
            break;
        }

        vec.clear();
    }

    if file.borrow().size() > 0 {
        nr_lines += 1;
    }
    println!("nr_lines {}", nr_lines)
}

///////////////////////////////////////////////////////////////////////////////////////////////////

fn test_insert(
    test_num: u32,
    file_size: usize,
    page_size: usize,
    insert_size: usize,
    nr_insert: usize,
) {
    use std::fs::File;
    use std::io::prelude::*;

    let filename = "/tmp/playground_insert_test";
    let mut file = File::create(filename).unwrap();

    // prepare file content
    println!("-- generating test file");
    let mut slc = Vec::with_capacity(file_size);
    for i in 0..file_size {
        if (i % (1024 * 1024 * 256)) == 0 {
            println!("-- @ bytes {}", i);
        }

        let val = if i % 100 == 0 {
            '\n' as u8
        } else {
            (('0' as i32) + (i as i32 % 10)) as u8
        };
        slc.push(val);
    }
    file.write_all(slc.as_slice()).unwrap();
    file.sync_all().unwrap();
    drop(slc);

    println!("-- mapping the test file");
    let file = match MappedFile::new(filename, page_size) {
        Some(file) => file,
        None => panic!("cannot map file"),
    };

    println!("-- testing insert");

    let mut insert_data = Vec::with_capacity(insert_size);
    for i in 0..insert_size {
        if i % 100 == 0 {
            insert_data.push('\n' as u8);
        } else {
            let val = (('0' as i32) + (i as i32 % 10)) as u8;
            insert_data.push(val);
        }
    }

    println!("-- insert data size = {} bytes", insert_data.len());
    if false {
        for off in 0..nr_insert {
            // let fsz = file.as_ref().borrow().size();
            // println!("fsz = {}", fsz);
            let mut it = MappedFile::iter_from(&file, off as u64);
            MappedFile::insert(&mut it, &insert_data.as_slice());
        }
    }

    println!("-- generate random offsets");

    let mut indexes = Vec::new();
    let between = Range::new(0 as u64, (file_size + 1) as u64);
    let mut rng = rand::thread_rng();

    for _ in 0..nr_insert {
        let a = between.ind_sample(&mut rng);
        //        println!("random index {}", a);
        indexes.push(a);
    }

    println!("-- insert {} times", indexes.len());
    for off in &indexes {
        let mut it = MappedFile::iter_from(&file, *off as u64);
        MappedFile::insert(&mut it, &insert_data.as_slice());
    }

    return;

    println!("-- print data {{");
    let mut count: u64 = 0;
    for i in MappedFile::iter(&file) {
        print!("{}", *i as char);
        count += 1;
    }
    println!("}}");
    println!(" count = {}", count);

    //     println!("-- testing insert 2");
    //      MappedFile::insert(&mut it0, &[1, 2, 3]);

    use std::io;

    if !true {
        println!("Hit [Enter] to stop");
        let mut stop = String::new();
        io::stdin().read_line(&mut stop).expect("something");
    }
}

fn test_remove(test_num: u32, file_size: usize, page_size: usize, nr_remove: usize, offset: u64) {
    use std::fs::File;
    use std::io::prelude::*;

    let filename = "/tmp/playground_insert_test";
    let mut file = File::create(filename).unwrap();

    // prepare file content
    println!("-- generating test file");
    let mut slc = Vec::with_capacity(file_size);
    for i in 0..file_size {
        if (i % (1024 * 1024 * 256)) == 0 {
            println!("-- @ bytes {}", i);
        }

        let val = if i % 100 == 0 {
            '\n' as u8
        } else {
            (('0' as i32) + (i as i32 % 10)) as u8
        };
        slc.push(val);
    }
    file.write_all(slc.as_slice()).unwrap();
    file.sync_all().unwrap();
    drop(slc);

    println!("-- mapping the test file");
    let file = match MappedFile::new(filename, page_size) {
        Some(file) => file,
        None => panic!("cannot map file"),
    };

    println!(
        "-- testing remove {} @ {} from {}",
        nr_remove, offset, file_size
    );
    let mut it = MappedFile::iter_from(&file, offset);
    MappedFile::remove(&mut it, nr_remove);

    println!("-- file.size() {}", file.as_ref().borrow().size());

    return;
    println!("-- print data {{");
    let mut count: u64 = 0;
    for i in MappedFile::iter(&file) {
        print!("{}", *i as char);
        count += 1;
    }
    println!("}}");
    println!(" count = {}", count);

    use std::io;

    if !true {
        println!("Hit [Enter] to stop");
        let mut stop = String::new();
        io::stdin().read_line(&mut stop).expect("something");
    }
}

fn main() {
    let file_size = 1024 * 1024 * 1024 * 2;
    let page_size = 4096;
    let nr_remove = file_size - 4096 * 2;
    let offset = page_size as u64;
    test_remove(0, file_size, page_size, nr_remove, offset);

    return;
    test_remove(0, 1024 * 1024 * 1024 * 1, 4096, 4096 * 2, 0);

    return;
    test_remove(0, 4096 * 4, 4096, 4096 * 2, 0);

    for i in 0..4096 + 1 {
        test_remove(0, 4096, 4096, i, 0);
    }

    let max_file_size: usize = 1024 * 1024 * 100;
    let max_page_size = 4096 * 2;
    let max_insert_size = 4;
    let max_nr_insert = 10;

    let mut test_num = 0;
    let mut file_size = 0;

    while file_size < max_file_size {
        let mut page_size = 4096;
        while page_size < max_page_size {
            let mut insert_size = 1;
            while insert_size < max_insert_size {
                let mut nr_insert = 1;
                while nr_insert < max_nr_insert {
                    println!("------------------------------------");
                    println!("test_num {}", test_num);
                    println!("file_size {}", file_size);
                    println!("page_size {}", page_size);
                    println!("insert_size {}", insert_size);

                    test_insert(test_num, file_size, page_size, insert_size, nr_insert);

                    test_num += 1;
                    nr_insert += 1;
                }
                insert_size += 1;
            }
            page_size += 4096;
        }
        file_size += 1024 * 1024;
    }
}
