use anyhow::{bail, Context, Result};
use std::{ffi::OsString, path::PathBuf};
fn usage() {
    println!("daynight: [command_option] [-i jsonFile] [-e heicFile]\nCommand options are:\n -h, --help    show this message and exit\n -v, --version show program version and exit\n -o FILE       output file name (default is 'output.heic')\n -i FILE       input .json wallpaper description\n -e FILE       input image to extract metadata");
}
fn run() -> Result<()> {
    let mut args = std::env::args_os().skip(1).peekable();
    if args.peek().is_none() {
        usage();
        bail!("Missing input file");
    }
    let mut input = None;
    let mut output = None;
    let mut extract = false;
    while let Some(arg) = args.next() {
        match arg.to_str() {
            Some("-h" | "--help") => {
                usage();
                return Ok(());
            }
            Some("-v" | "--version") => {
                println!("{}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some("-i" | "-e") => {
                if input.is_some() {
                    bail!("Specify exactly one of -i or -e");
                }
                extract = arg == "-e";
                input = Some(PathBuf::from(next_value(&mut args, &arg)?));
            }
            Some("-o") => output = Some(PathBuf::from(next_value(&mut args, &arg)?)),
            _ => bail!("Unknown option: {}", arg.to_string_lossy()),
        }
    }
    let input = input.context("Specify an input with -i or -e")?;
    if extract {
        daynight::extract(&input, output.as_deref())
    } else {
        daynight::generate(&input, &output.unwrap_or_else(|| "output.heic".into()))
    }
}
fn next_value(args: &mut impl Iterator<Item = OsString>, option: &OsString) -> Result<OsString> {
    let value = args
        .next()
        .with_context(|| format!("Missing value for {}", option.to_string_lossy()))?;
    if value.to_string_lossy().starts_with('-') {
        bail!("Missing value for {}", option.to_string_lossy());
    }
    Ok(value)
}
fn main() {
    if let Err(error) = run() {
        eprintln!("Error: {error:#}");
        std::process::exit(1);
    }
}
