//! HEIC support for Linux and Windows, with the same Apple XMP/plist contract
//! as the macOS backend. Native codec availability is checked at runtime.
use crate::metadata::{decode, parse_date, Plan, Sequence};
use anyhow::{bail, ensure, Context, Result};
use exif::{In, Tag, Value};
use image::{DynamicImage, ImageDecoder, ImageReader, RgbaImage};
use libheif_rs::{
    Channel, ColorProfileRaw, ColorSpace, CompressionFormat, DecodingOptions, EncoderQuality,
    EncodingOptions, HeifContext, Image, ImageHandle, LibHeif, RgbChroma, StreamReader,
};
use std::{
    fs::File,
    io::{BufReader, Write},
    path::Path,
};

const APPLE_NAMESPACE: &str = "http://ns.apple.com/namespace/1.0/";

fn libheif() -> Result<LibHeif> {
    let heif = LibHeif::new_checked().context("Cannot initialize libheif codec plugins")?;
    // Windows release artifacts carry optional codec plugins beside the binaries.
    // System package installations continue to use libheif's normal search paths.
    #[cfg(windows)]
    {
        let executable = std::env::current_exe()?;
        if let Some(parent) = executable.parent() {
            let plugins = parent.join("plugins");
            if plugins.is_dir() {
                heif.load_plugins(&plugins)
                    .context("Cannot load bundled HEIC codecs")?;
            }
        }
    }
    Ok(heif)
}

fn context(input: &Path) -> Result<HeifContext<'static>> {
    let file = File::open(input).with_context(|| format!("Cannot open {}", input.display()))?;
    let size = file.metadata()?.len();
    HeifContext::read_from_reader(Box::new(StreamReader::new(BufReader::new(file), size)))
        .with_context(|| format!("Cannot read HEIC {}", input.display()))
}

pub fn generate(plan: &Plan<'_>, base: &Path, output: &Path) -> Result<()> {
    ensure!(
        !plan.images.is_empty(),
        "Wallpaper must contain at least one image"
    );
    for name in &plan.images {
        let path = base.join(name);
        ensure!(path.is_file(), "Cannot read image {}", path.display());
        ensure!(
            std::path::absolute(&path)? != std::path::absolute(output)?,
            "Output must not replace an input image"
        );
    }
    let heif = libheif()?;
    let mut encoder = heif
        .encoder_for_format(CompressionFormat::Hevc)
        .context("No HEVC encoder available: install libheif with an HEVC encoder such as x265")?;
    encoder.set_quality(EncoderQuality::Lossy(100))?;
    let mut ctx = HeifContext::new()?;
    for (index, name) in plan.images.iter().enumerate() {
        let path = base.join(name);
        eprintln!(
            "Adding image {}/{}: {}",
            index + 1,
            plan.images.len(),
            path.display()
        );
        let image = load_image(&heif, &path)?;
        let mut options = EncodingOptions::new()?;
        options.set_mac_os_compatibility_workaround(true);
        let mut handle = ctx.encode_image(&image, &mut encoder, Some(options))?;
        if index == 0 {
            ctx.set_primary_image(&mut handle)?;
            ctx.add_xmp_metadata(&handle, xmp(plan).as_bytes())?;
        }
    }
    let parent = output
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or(Path::new("."));
    let mut temp =
        tempfile::NamedTempFile::new_in(parent).context("Cannot create temporary output")?;
    // Use Rust file IO on Windows for Unicode paths (the C API takes a narrow
    // filename). Unix can write directly without buffering the complete HEIC.
    if cfg!(windows) || temp.path().to_str().is_none() {
        temp.write_all(&ctx.write_to_bytes()?)?;
    } else {
        ctx.write_to_file(temp.path().to_str().unwrap())?;
    }
    temp.persist(output)
        .with_context(|| format!("Cannot save {}", output.display()))?;
    eprintln!("Saved {}", output.display());
    Ok(())
}

fn xmp(plan: &Plan<'_>) -> String {
    // Both the key and base64 are generated internally, with no XML metacharacters.
    format!(
        r#"<x:xmpmeta xmlns:x="adobe:ns:meta/"><rdf:RDF xmlns:rdf="http://www.w3.org/1999/02/22-rdf-syntax-ns#"><rdf:Description rdf:about="" xmlns:apple_desktop="{APPLE_NAMESPACE}" apple_desktop:{}="{}"/></rdf:RDF></x:xmpmeta>"#,
        plan.key,
        plan.base64()
    )
}

fn xmp_tags(bytes: &[u8]) -> Result<Vec<(String, String)>> {
    let text = std::str::from_utf8(bytes).context("XMP is not UTF-8")?;
    let doc = roxmltree::Document::parse(text.trim_end_matches('\0')).context("Invalid XMP XML")?;
    let mut result = Vec::new();
    for node in doc.descendants().filter(|n| n.is_element()) {
        for attribute in node.attributes() {
            if attribute.namespace() == Some(APPLE_NAMESPACE) {
                result.push((
                    format!("apple_desktop:{}", attribute.name()),
                    attribute.value().to_owned(),
                ));
            }
        }
        if node.tag_name().namespace() == Some(APPLE_NAMESPACE) {
            if let Some(value) = node.text() {
                result.push((
                    format!("apple_desktop:{}", node.tag_name().name()),
                    value.to_owned(),
                ));
            }
        }
    }
    Ok(result)
}

pub fn tags(input: &Path) -> Result<Vec<(String, String)>> {
    let _heif = libheif()?;
    let ctx = context(input)?;
    tags_for_handle(&ctx.primary_image_handle()?)
}
fn tags_for_handle(handle: &ImageHandle) -> Result<Vec<(String, String)>> {
    let count = handle.number_of_metadata_blocks(0).max(0) as usize;
    let mut ids = vec![0; count];
    let count = handle.metadata_block_ids(&mut ids, 0);
    let mut tags = Vec::new();
    for id in ids.into_iter().take(count) {
        if handle.metadata_content_type(id) == Some("application/rdf+xml") {
            tags.extend(xmp_tags(&handle.metadata(id)?)?);
        }
    }
    Ok(tags)
}

fn decode_image(heif: &LibHeif, handle: &ImageHandle) -> Result<RgbaImage> {
    let mut options = DecodingOptions::new().context("Cannot allocate decoding options")?;
    options.set_strict_decoding(true);
    let decoded = heif.decode(handle, ColorSpace::Rgb(RgbChroma::Rgba), Some(options))?;
    ensure!(
        decoded.width() > 0 && decoded.height() > 0,
        "Decoded image is empty"
    );
    let plane = decoded
        .planes()
        .interleaved
        .context("Missing decoded RGB plane")?;
    let row_bytes = decoded.width() as usize * 4;
    ensure!(
        plane.stride >= row_bytes
            && plane.data.len() >= plane.stride * (decoded.height() as usize - 1) + row_bytes,
        "Invalid decoded plane dimensions"
    );
    let mut pixels = RgbaImage::new(decoded.width(), decoded.height());
    for (output, input) in pixels
        .as_mut()
        .chunks_exact_mut(row_bytes)
        .zip(plane.data.chunks(plane.stride))
    {
        output.copy_from_slice(
            input
                .get(..row_bytes)
                .context("Invalid decoded row stride")?,
        );
    }
    Ok(pixels)
}

fn load_image(heif: &LibHeif, path: &Path) -> Result<Image> {
    let reader = ImageReader::open(path)?.with_guessed_format()?;
    let (pixels, profile) = if reader.format().is_some() {
        let mut decoder = reader
            .into_decoder()
            .with_context(|| format!("Cannot decode {}", path.display()))?;
        let orientation = decoder.orientation()?;
        let profile = decoder.icc_profile()?;
        let mut image = DynamicImage::from_decoder(decoder)?;
        image.apply_orientation(orientation);
        (
            image.to_rgba8(),
            profile.map(|p| ColorProfileRaw::new(libheif_rs::color_profile_types::PROF, p)),
        )
    } else {
        let ctx = context(path)?;
        let handle = ctx.primary_image_handle()?;
        (decode_image(heif, &handle)?, handle.color_profile_raw())
    };
    let (width, height) = pixels.dimensions();
    let mut image = Image::new(width, height, ColorSpace::Rgb(RgbChroma::Rgba))?;
    image.create_plane(Channel::Interleaved, width, height, 8)?;
    let plane = image
        .planes_mut()
        .interleaved
        .context("Cannot allocate RGB plane")?;
    let row_bytes = width as usize * 4;
    for (input, output) in pixels
        .as_raw()
        .chunks_exact(row_bytes)
        .zip(plane.data.chunks_mut(plane.stride))
    {
        output[..row_bytes].copy_from_slice(input);
    }
    if let Some(profile) = profile {
        image.set_color_profile_raw(&profile)?;
    }
    Ok(image)
}

pub fn location(input: &Path) -> Result<(f64, f64, f64)> {
    let file = File::open(input).with_context(|| format!("Cannot open {}", input.display()))?;
    let exif = exif::Reader::new()
        .read_from_container(&mut BufReader::new(file))
        .context("Cannot read EXIF metadata")?;
    let latitude = coordinate(&exif, Tag::GPSLatitude, Tag::GPSLatitudeRef, "N", "S", 90.0)?;
    let longitude = coordinate(
        &exif,
        Tag::GPSLongitude,
        Tag::GPSLongitudeRef,
        "E",
        "W",
        180.0,
    )?;
    let (creation, offset_tag) = [
        (Tag::DateTimeOriginal, Tag::OffsetTimeOriginal),
        (Tag::DateTimeDigitized, Tag::OffsetTimeDigitized),
        (Tag::DateTime, Tag::OffsetTime),
    ]
    .into_iter()
    .find_map(|(tag, offset)| ascii(&exif, tag).map(|value| (value, offset)))
    .context("Missing creation date")?;
    let date = if let Some(offset) = ascii(&exif, offset_tag) {
        chrono::DateTime::parse_from_str(&format!("{creation}{offset}"), "%Y:%m:%d %H:%M:%S%:z")
            .context("Invalid EXIF capture time or timezone offset")?
            .with_timezone(&chrono::Local)
    } else {
        parse_date(&creation)?
    };
    Ok((latitude, longitude, date.timestamp_millis() as f64 / 1000.0))
}
fn ascii(exif: &exif::Exif, tag: Tag) -> Option<String> {
    match &exif.get_field(tag, In::PRIMARY)?.value {
        Value::Ascii(values) => Some(
            std::str::from_utf8(values.first()?)
                .ok()?
                .trim_end_matches('\0')
                .to_owned(),
        ),
        _ => None,
    }
}
fn coordinate(
    exif: &exif::Exif,
    tag: Tag,
    reference: Tag,
    positive: &str,
    negative: &str,
    limit: f64,
) -> Result<f64> {
    let field = exif
        .get_field(tag, In::PRIMARY)
        .with_context(|| format!("Missing {tag}"))?;
    let Value::Rational(parts) = &field.value else {
        bail!("Invalid {tag}");
    };
    ensure!(
        parts.len() == 3 && parts.iter().all(|r| r.denom != 0),
        "Invalid {tag}"
    );
    let (degrees, minutes, seconds) = (parts[0].to_f64(), parts[1].to_f64(), parts[2].to_f64());
    ensure!(minutes < 60.0 && seconds < 60.0, "Invalid {tag}");
    let value = degrees + minutes / 60.0 + seconds / 3600.0;
    ensure!(value.is_finite() && value <= limit, "Invalid {tag}");
    match ascii(exif, reference).as_deref() {
        Some(r) if r == positive => Ok(value),
        Some(r) if r == negative => Ok(-value),
        _ => bail!("Missing or invalid {reference}"),
    }
}

pub fn inspect(input: &Path, output: &Path) -> Result<()> {
    let heif = libheif()?;
    let ctx = context(input)?;
    let ids = ctx.image_ids();
    let primary_id = ctx.primary_image_handle()?.item_id();
    let primary = ids
        .iter()
        .position(|id| *id == primary_id)
        .context("Missing primary image")?;
    let tags = tags_for_handle(&ctx.primary_image_handle()?)?;
    let payload = decode(
        &tags
            .iter()
            .find(|(key, _)| key == "apple_desktop:h24")
            .context("Missing h24 metadata")?
            .1,
    )?;
    let sequence: Sequence = plist::from_bytes(&payload)?;
    let ap = sequence.ap.context("Missing appearance metadata")?;
    ensure!(
        ap.l < ids.len() && ap.d < ids.len(),
        "Invalid Light/Dark indices"
    );
    let previews = [
        ("primary.png", primary),
        ("light.png", ap.l),
        ("dark.png", ap.d),
    ];
    let mut frames = Vec::with_capacity(ids.len());
    for (index, id) in ids.iter().enumerate() {
        let handle = ctx.image_handle(*id)?;
        let pixels = decode_image(&heif, &handle)?;
        ensure!(
            pixels.dimensions() == (handle.width(), handle.height()),
            "Decoded frame dimensions do not match properties"
        );
        for (name, preview) in previews {
            if preview == index {
                // create_new prevents overwriting evidence from a prior run.
                let file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(output.join(name))?;
                let mut writer = std::io::BufWriter::new(file);
                pixels.write_to(&mut writer, image::ImageFormat::Png)?;
                writer.flush()?;
            }
        }
        frames.push(serde_json::json!({"index": index, "width": pixels.width(), "height": pixels.height(), "decoded": true}));
    }
    let metadata: serde_json::Value = plist::from_bytes(&payload)?;
    let report = serde_json::json!({"count": ids.len(), "primaryIndex": primary, "frames": frames,
        "metadata": metadata, "previews": previews.map(|(file, index)| serde_json::json!({"file": file, "index": index})),
        "visualReview": "not performed by this script"});
    for (name, data) in [
        ("metadata.plist", payload),
        ("container.json", serde_json::to_vec_pretty(&report)?),
    ] {
        std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(output.join(name))?
            .write_all(&data)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn xmp_matches_namespace_with_attributes_and_elements() {
        let xml = br#"<x xmlns:a="http://ns.apple.com/namespace/1.0/" a:h24="one"><a:apr>two</a:apr><h24>ignored</h24></x>"#;
        assert_eq!(
            xmp_tags(xml).unwrap(),
            [
                ("apple_desktop:h24".into(), "one".into()),
                ("apple_desktop:apr".into(), "two".into())
            ]
        );
        assert!(xmp_tags(b"malformed XML").is_err());
    }
    #[test]
    fn rotated_photo_decodes_upright_at_full_resolution() {
        let heif = libheif().unwrap();
        let image = load_image(
            &heif,
            &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oriented.jpg"),
        )
        .unwrap();
        assert_eq!((image.width(), image.height()), (32, 16));
    }
}
