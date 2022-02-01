use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

const SZ: usize = 1024 * 32;

fn main() -> std::io::Result<()> {
    let file = File::create("all_utf8.bin")?;

    let mut buf_writer = BufWriter::with_capacity(SZ, file);

    let mut d: [u8; 256] = [0; 256];
    for i in 0..256 {
        d[i] = i as u8;
    }
    buf_writer.write_all(&d)?;

    let mut d: [u8; 256 * 2] = [0; 256 * 2];
    for j in 0..256 {
        for i in 0..256 {
            d[(i * 2) + 0] = i as u8;
            d[(i * 2) + 1] = j as u8;
        }
        buf_writer.write_all(&d)?;
    }

    let mut d: [u8; 256 * 3] = [0; 256 * 3];
    for k in 0..256 {
        for j in 0..256 {
            for i in 0..256 {
                d[(i * 3) + 0] = i as u8;
                d[(i * 3) + 1] = j as u8;
                d[(i * 3) + 2] = k as u8;
            }
            buf_writer.write_all(&d)?;
        }
    }

    let mut d: [u8; 256 * 4] = [0; 256 * 4];
    for l in 0..256 {
        for k in 0..256 {
            for j in 0..256 {
                for i in 0..256 {
                    d[(i * 4) + 0] = i as u8;
                    d[(i * 4) + 1] = j as u8;
                    d[(i * 4) + 2] = k as u8;
                    d[(i * 4) + 3] = l as u8;
                }
                buf_writer.write_all(&d)?;
            }
        }
    }
    Ok(())
}
