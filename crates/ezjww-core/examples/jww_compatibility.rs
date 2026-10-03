//! Generate the native compatibility matrix into a new directory.
#[path = "support/writer_cases.rs"]
mod writer_cases;
#[path = "support/writer_compatibility_cases.rs"]
mod writer_compatibility_cases;

use std::{env, fs, io::Write, path::Path};

fn main() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<_> = env::args().collect();
    if args.len() != 2 {
        return Err("usage: jww_compatibility <new-output-directory>".into());
    }
    let output = Path::new(&args[1]);
    fs::create_dir(output)?;
    for &name in writer_compatibility_cases::CASES {
        let doc = writer_compatibility_cases::drawing(name).unwrap();
        let bytes = ezjww_core::to_jww_bytes(&doc)?;
        fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(output.join(format!("{name}.jww")))?
            .write_all(&bytes)?;
        println!(
            "{name}: {} entities, {} bytes",
            doc.entities.len(),
            bytes.len()
        );
    }
    Ok(())
}
