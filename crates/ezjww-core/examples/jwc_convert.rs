//! Development corpus exporter for independent DXF/preview validation, not the
//! Python/CLI integration planned in P5. Usage: jwc_convert INPUT_DIR OUTPUT_DIR
use ezjww_core::jwc::{convert_jwc_document, JwcConvertOptions, JwcCoordinateSpace};
use ezjww_core::{parse_jwc_document, DxfTargetVersion};
use std::{error::Error, fs, path::PathBuf};

fn main() -> Result<(), Box<dyn Error>> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.len() != 2 {
        return Err("usage: jwc_convert INPUT_DIR OUTPUT_DIR".into());
    }
    let output = PathBuf::from(&args[1]);
    fs::create_dir_all(&output)?;
    let mut paths: Vec<_> = fs::read_dir(&args[0])?
        .map(|e| e.map(|e| e.path()))
        .collect::<Result<_, _>>()?;
    paths.sort();
    let mut cases = Vec::new();
    for path in paths {
        if path
            .extension()
            .is_none_or(|e| !e.eq_ignore_ascii_case("jwc"))
        {
            continue;
        }
        let id = path
            .file_stem()
            .ok_or("missing filename")?
            .to_string_lossy();
        let doc = match parse_jwc_document(&fs::read(&path)?) {
            Ok(doc) => doc,
            Err(e) => {
                cases.push(serde_json::json!({"id":id,"error":e.to_string(),"byte_offset":e.byte_offset()}));
                continue;
            }
        };
        for (name, coordinates) in [
            ("paper", JwcCoordinateSpace::PaperMillimeters),
            ("model", JwcCoordinateSpace::ModelMillimeters),
        ] {
            let result = convert_jwc_document(
                &doc,
                JwcConvertOptions {
                    coordinates,
                    ..Default::default()
                },
            )?;
            result.write_to_file(
                output.join(format!("{id}_{name}.dxf")),
                DxfTargetVersion::Ac1015,
            )?;
            result.write_to_file(
                output.join(format!("{id}_{name}_ac1024.dxf")),
                DxfTargetVersion::Ac1024,
            )?;
            fs::write(
                output.join(format!("{id}_{name}.json")),
                serde_json::to_vec_pretty(&result)?,
            )?;
        }
        cases.push(serde_json::json!({"id":id,"entities":doc.entities.len(),"status":"converted"}));
    }
    fs::write(
        output.join("exports.json"),
        serde_json::to_vec_pretty(&cases)?,
    )?;
    println!(
        "Exported {} cases (see exports.json for explicit rejections)",
        cases.len()
    );
    Ok(())
}
