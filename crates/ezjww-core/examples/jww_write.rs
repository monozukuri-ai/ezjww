//! Generate the two minimal writer acceptance cases without overwriting a file.
use ezjww_core::{to_jww_bytes, Coord2D, JwwWriteDocument};
use std::{env, fs::OpenOptions, io::Write};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 3 || !matches!(args[1].as_str(), "empty" | "line") {
        return Err("usage: jww_write <empty|line> <output.jww>".into());
    }
    let mut doc = JwwWriteDocument::default();
    if args[1] == "line" {
        doc.add_line(Coord2D::new(0.0, 0.0), Coord2D::new(100.0, 0.0));
    }
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
