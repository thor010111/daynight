//! Dynamic wallpaper schedules and native macOS ImageIO encoding.
#[cfg(not(target_os = "macos"))]
compile_error!("daynight requires macOS and Apple's ImageIO HEIC encoder");

pub mod metadata;
pub mod native;
pub mod sun;

use anyhow::{Context, Result};
use std::path::Path;

pub fn generate(input: &Path, output: &Path) -> Result<()> {
    let pictures: Vec<metadata::PictureInfo> = serde_json::from_reader(
        std::fs::File::open(input).with_context(|| format!("Cannot open {}", input.display()))?,
    )
    .context("Invalid wallpaper JSON")?;
    let plan = metadata::Plan::new(&pictures)?;
    native::generate(&plan, input.parent().unwrap_or(Path::new(".")), output)
}

pub fn extract(input: &Path, output: Option<&Path>) -> Result<()> {
    for (key, value) in native::tags(input)? {
        println!("---------------------------------------------------\nMetadata key: {key}");
        if matches!(
            key.as_str(),
            "apple_desktop:solar" | "apple_desktop:h24" | "apple_desktop:apr"
        ) {
            let data = metadata::decode(&value)?;
            if let Some(output) = output {
                std::fs::write(output, &data)
                    .with_context(|| format!("Cannot write {}", output.display()))?;
                println!("\tSaved plist file: {}", output.display());
            }
            if key.ends_with(":apr") {
                print_appearance(&plist::from_bytes(&data)?);
            } else {
                let seq: metadata::Sequence = plist::from_bytes(&data)?;
                if let Some(ap) = seq.ap {
                    print_appearance(&ap);
                }
                if let Some(items) = seq.si {
                    println!("[SOLAR]");
                    for item in items {
                        println!(
                            "\timage index: {}, azimuth: {}, altitude: {}",
                            item.i, item.z, item.a
                        );
                    }
                }
                if let Some(items) = seq.ti {
                    println!("[TIME]");
                    for item in items {
                        println!("\timage index: {}, time: {}", item.i, item.t);
                    }
                }
            }
        } else {
            println!("\tvalue: {value}");
        }
    }
    Ok(())
}

fn print_appearance(ap: &metadata::Appearance) {
    println!(
        "[APPERANCE]\n\timage index: {}, dark\n\timage index: {}, light",
        ap.d, ap.l
    );
}
