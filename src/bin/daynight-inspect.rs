//! Native companion to the wallpaper skill's Python verifier.
use anyhow::{ensure, Result};
use std::path::PathBuf;
fn run() -> Result<()> {
    let args: Vec<_> = std::env::args_os().skip(1).map(PathBuf::from).collect();
    ensure!(
        args.len() == 2,
        "Usage: daynight-inspect HEIC OUTPUT_DIRECTORY (use verify_wallpaper.py)"
    );
    ensure!(
        args[1].is_dir() && std::fs::read_dir(&args[1])?.next().is_none(),
        "Output directory must exist and be empty"
    );
    daynight::native::inspect(&args[0], &args[1])
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
