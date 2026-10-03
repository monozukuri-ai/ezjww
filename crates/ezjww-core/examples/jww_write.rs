//! Generate writer acceptance cases without overwriting a file.
use ezjww_core::to_jww_bytes;
#[path = "support/writer_cases.rs"]
mod writer_cases;
use std::{env, fs::OpenOptions, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 {
        return Err("usage: jww_write <empty|line|basic|settings|unicode> <output.jww>".into());
    }
    let doc = writer_cases::drawing(&args[1]).ok_or("unknown acceptance case")?;
    let data = to_jww_bytes(&doc)?;
    OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(&args[2])?
        .write_all(&data)?;
    println!(
        "{}: {} bytes, {} entities",
        args[2],
        data.len(),
        doc.entities.len()
    );
    Ok(())
}
