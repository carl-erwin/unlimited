use std::fs::File;
use std::io::BufWriter;
use std::io::Write;

fn main() -> std::io::Result<()> {
    let file = File::create("all_utf8.bin")?;

    let mut buf_writer = BufWriter::new(file);

    for i in 0..0xff {
        buf_writer.write(&[i])?;
    }

    for j in 0..0xff {
        for i in 0..0xff {
            buf_writer.write(&[i])?;
            buf_writer.write(&[j])?;
        }
    }

    for k in 0..0xff {
        for j in 0..0xff {
            for i in 0..0xff {
                buf_writer.write(&[i])?;
                buf_writer.write(&[j])?;
                buf_writer.write(&[k])?;
            }
        }
    }

    for l in 0..0xff {
        for k in 0..0xff {
            for j in 0..0xff {
                for i in 0..0xff {
                    buf_writer.write(&[i])?;
                    buf_writer.write(&[j])?;
                    buf_writer.write(&[k])?;
                    buf_writer.write(&[l])?;
                }
            }
        }
    }

    Ok(())
}
