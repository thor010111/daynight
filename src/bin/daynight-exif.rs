use anyhow::{bail, Result};
use serde::Serialize;
use std::path::PathBuf;
#[derive(Serialize)]
struct Item {
    #[serde(rename = "fileName")]
    file_name: String,
    #[serde(flatten)]
    position: daynight::sun::SunPosition,
}
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).collect();
    if args.iter().any(|s| s == "-h" || s == "--help") || args.is_empty() {
        println!("daynight-exif: [file1] [file2]\n -h, --help    show this message and exit\n -v, --version show program version and exit");
        if args.is_empty() {
            bail!("Missing image files");
        }
        return Ok(());
    }
    if args.iter().any(|s| s == "-v" || s == "--version") {
        println!("{}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    let mut items = Vec::with_capacity(args.len());
    for arg in args {
        let path = PathBuf::from(arg);
        let (lat, lon, seconds) = daynight::native::location(&path)?;
        items.push(Item {
            file_name: path
                .to_str()
                .ok_or_else(|| anyhow::anyhow!("Image path is not valid UTF-8"))?
                .into(),
            position: daynight::sun::position(lat, lon, seconds),
        });
    }
    println!("Output:\n{}", serde_json::to_string_pretty(&items)?);
    Ok(())
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
