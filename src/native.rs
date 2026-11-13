//! Minimal ImageIO bridge. Every Create/Copy result is owned by `Owned` and released
//! on all return paths; borrowed Core Foundation values never escape their owner.
use crate::metadata::{parse_date, Plan};
use anyhow::{bail, ensure, Context, Result};
use core_foundation::array::{CFArrayGetCount, CFArrayGetValueAtIndex, CFArrayRef};
use core_foundation::base::{CFGetTypeID, CFRelease};
use core_foundation::dictionary::{CFDictionaryGetTypeID, CFDictionaryGetValue, CFDictionaryRef};
use core_foundation::number::{kCFNumberFloat64Type, CFNumberGetTypeID, CFNumberGetValue};
use core_foundation::string::{CFStringGetTypeID, CFStringRef};
use core_foundation::{
    base::TCFType, boolean::CFBoolean, dictionary::CFDictionary, number::CFNumber,
    string::CFString, url::CFURL,
};
use std::{
    ffi::c_void,
    path::{Path, PathBuf},
    ptr::{null, null_mut, NonNull},
    sync::atomic::{AtomicU64, Ordering},
};

type Ptr = *const c_void;
#[link(name = "ImageIO", kind = "framework")]
extern "C" {
    fn CGImageSourceCreateWithURL(url: Ptr, options: Ptr) -> Ptr;
    fn CGImageSourceCreateImageAtIndex(source: Ptr, index: usize, options: Ptr) -> Ptr;
    fn CGImageSourceCreateThumbnailAtIndex(source: Ptr, index: usize, options: Ptr) -> Ptr;
    fn CGImageSourceCopyMetadataAtIndex(source: Ptr, index: usize, options: Ptr) -> Ptr;
    fn CGImageSourceCopyPropertiesAtIndex(source: Ptr, index: usize, options: Ptr) -> Ptr;
    fn CGImageDestinationCreateWithURL(url: Ptr, kind: Ptr, count: usize, options: Ptr) -> Ptr;
    fn CGImageDestinationAddImage(destination: Ptr, image: Ptr, options: Ptr);
    fn CGImageDestinationAddImageAndMetadata(
        destination: Ptr,
        image: Ptr,
        metadata: Ptr,
        options: Ptr,
    );
    fn CGImageDestinationFinalize(destination: Ptr) -> bool;
    fn CGImageMetadataCreateMutable() -> Ptr;
    fn CGImageMetadataRegisterNamespaceForPrefix(
        metadata: Ptr,
        namespace: Ptr,
        prefix: Ptr,
        error: *mut Ptr,
    ) -> bool;
    fn CGImageMetadataTagCreate(
        namespace: Ptr,
        prefix: Ptr,
        name: Ptr,
        kind: i32,
        value: Ptr,
    ) -> Ptr;
    fn CGImageMetadataSetTagWithPath(metadata: Ptr, parent: Ptr, path: Ptr, tag: Ptr) -> bool;
    fn CGImageMetadataCopyTags(metadata: Ptr) -> CFArrayRef;
    fn CGImageMetadataTagCopyPrefix(tag: Ptr) -> Ptr;
    fn CGImageMetadataTagCopyName(tag: Ptr) -> Ptr;
    fn CGImageMetadataTagCopyValue(tag: Ptr) -> Ptr;
    static kCGImageDestinationLossyCompressionQuality: CFStringRef;
}

struct Owned(NonNull<c_void>);
impl Owned {
    /// The caller supplies a Create/Copy result with one owned retain count.
    unsafe fn new(value: Ptr, message: &str) -> Result<Self> {
        Ok(Self(
            NonNull::new(value.cast_mut()).context(message.to_owned())?,
        ))
    }
    fn ptr(&self) -> Ptr {
        self.0.as_ptr()
    }
}
impl Drop for Owned {
    fn drop(&mut self) {
        unsafe { CFRelease(self.ptr()) }
    }
}
fn string(value: Ptr) -> Option<String> {
    if value.is_null() {
        return None;
    }
    // Type-check before interpreting a CF value as a string.
    unsafe {
        (CFGetTypeID(value) == CFStringGetTypeID())
            .then(|| CFString::wrap_under_get_rule(value as CFStringRef).to_string())
    }
}
fn url(path: &Path) -> Result<CFURL> {
    let absolute = std::path::absolute(path)?;
    CFURL::from_path(&absolute, false).with_context(|| format!("Invalid path: {}", path.display()))
}
fn source(path: &Path) -> Result<Owned> {
    let url = url(path)?;
    unsafe {
        Owned::new(
            CGImageSourceCreateWithURL(url.as_CFTypeRef(), null()),
            &format!("Cannot read image {}", path.display()),
        )
    }
}

// Use a private sibling directory so failures clean up ImageIO scratch files and
// leave an existing output intact. The final rename stays on the same filesystem.
// Never reserve a predictable
// directory without atomic creation, or delete a path we do not own.
struct TemporaryOutput {
    directory: PathBuf,
    file: PathBuf,
}
impl TemporaryOutput {
    fn new(output: &Path) -> Result<Self> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        let parent = output
            .parent()
            .filter(|p| !p.as_os_str().is_empty())
            .unwrap_or(Path::new("."));
        for _ in 0..100 {
            let path = parent.join(format!(
                ".daynight-{}-{}",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            use std::os::unix::fs::DirBuilderExt;
            match std::fs::DirBuilder::new().mode(0o700).create(&path) {
                Ok(_) => {
                    return Ok(Self {
                        file: path.join("output.heic"),
                        directory: path,
                    })
                }
                Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => continue,
                Err(e) => return Err(e).context("Cannot create output file"),
            }
        }
        bail!("Cannot reserve temporary output")
    }
}
impl Drop for TemporaryOutput {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.directory);
    }
}

pub fn generate(plan: &Plan<'_>, base: &Path, output: &Path) -> Result<()> {
    // Reject missing inputs before starting the expensive encoder.
    for name in &plan.images {
        let path = base.join(name);
        ensure!(path.is_file(), "Cannot read image {}", path.display());
        ensure!(
            std::path::absolute(&path)? != std::path::absolute(output)?,
            "Output must not replace an input image"
        );
    }
    let temp = TemporaryOutput::new(output)?;
    let target = url(&temp.file)?;
    let kind = CFString::new("public.heic");
    let namespace = CFString::new("http://ns.apple.com/namespace/1.0/");
    let prefix = CFString::new("apple_desktop");
    let name = CFString::new(plan.key);
    let value = CFString::new(&plan.base64());
    let path = CFString::new(&format!("apple_desktop:{}", plan.key));
    // SAFETY: all pointers below refer to live CF objects, indices are zero,
    // and every nullable Create/Copy result is checked before use.
    unsafe {
        let metadata = Owned::new(CGImageMetadataCreateMutable(), "Cannot create metadata")?;
        ensure!(
            CGImageMetadataRegisterNamespaceForPrefix(
                metadata.ptr(),
                namespace.as_CFTypeRef(),
                prefix.as_CFTypeRef(),
                null_mut()
            ),
            "Cannot register Apple desktop namespace"
        );
        let tag = Owned::new(
            CGImageMetadataTagCreate(
                namespace.as_CFTypeRef(),
                prefix.as_CFTypeRef(),
                name.as_CFTypeRef(),
                1,
                value.as_CFTypeRef(),
            ),
            "Cannot create wallpaper tag",
        )?;
        ensure!(
            CGImageMetadataSetTagWithPath(metadata.ptr(), null(), path.as_CFTypeRef(), tag.ptr()),
            "Cannot attach wallpaper tag"
        );
        let destination = Owned::new(
            CGImageDestinationCreateWithURL(
                target.as_CFTypeRef(),
                kind.as_CFTypeRef(),
                plan.images.len(),
                null(),
            ),
            "Cannot create HEIC destination",
        )?;
        let quality_key = CFString::wrap_under_get_rule(kCGImageDestinationLossyCompressionQuality);
        let options = CFDictionary::from_CFType_pairs(&[(quality_key, CFNumber::from(1.0))]);
        for (index, name) in plan.images.iter().enumerate() {
            let file = base.join(name);
            eprintln!(
                "Adding image {}/{}: {}",
                index + 1,
                plan.images.len(),
                file.display()
            );
            let source = source(&file)?;
            let image = decoded_image(&source, &file)?;
            if index == 0 {
                CGImageDestinationAddImageAndMetadata(
                    destination.ptr(),
                    image.ptr(),
                    metadata.ptr(),
                    options.as_CFTypeRef(),
                );
            } else {
                CGImageDestinationAddImage(destination.ptr(), image.ptr(), options.as_CFTypeRef());
            }
            // Source and decoded image are released after each frame.
        }
        ensure!(
            CGImageDestinationFinalize(destination.ptr()),
            "HEIC encoding failed (ImageIO could not finalize the container)"
        );
    }
    std::fs::rename(&temp.file, output)
        .with_context(|| format!("Cannot save {}", output.display()))?;
    eprintln!("Saved {}", output.display());
    Ok(())
}

// CGImageSourceCreateImageAtIndex leaves EXIF orientation in the properties.
// For rotated/mirrored photos, render the transform at full resolution so the
// desktop receives upright pixels, as it did through NSImage in the Swift CLI.
unsafe fn decoded_image(source: &Owned, file: &Path) -> Result<Owned> {
    let properties = Owned::new(
        CGImageSourceCopyPropertiesAtIndex(source.ptr(), 0, null()),
        "Cannot read image properties",
    )?;
    let orientation = number(get(properties.ptr(), "Orientation")).unwrap_or(1.0);
    let raw = if orientation > 1.0 {
        let width = number(get(properties.ptr(), "PixelWidth")).context("Missing image width")?;
        let height =
            number(get(properties.ptr(), "PixelHeight")).context("Missing image height")?;
        let options = CFDictionary::from_CFType_pairs(&[
            (
                CFString::new("kCGImageSourceCreateThumbnailFromImageAlways"),
                CFBoolean::true_value().into_CFType(),
            ),
            (
                CFString::new("kCGImageSourceCreateThumbnailWithTransform"),
                CFBoolean::true_value().into_CFType(),
            ),
            (
                CFString::new("kCGImageSourceThumbnailMaxPixelSize"),
                CFNumber::from(width.max(height) as i64).into_CFType(),
            ),
        ]);
        CGImageSourceCreateThumbnailAtIndex(source.ptr(), 0, options.as_CFTypeRef())
    } else {
        CGImageSourceCreateImageAtIndex(source.ptr(), 0, null())
    };
    Owned::new(raw, &format!("Cannot decode image {}", file.display()))
}

pub fn tags(input: &Path) -> Result<Vec<(String, String)>> {
    let source = source(input)?;
    unsafe {
        let metadata = Owned::new(
            CGImageSourceCopyMetadataAtIndex(source.ptr(), 0, null()),
            "No image metadata found",
        )?;
        let raw = CGImageMetadataCopyTags(metadata.ptr());
        if raw.is_null() {
            return Ok(Vec::new());
        }
        let _tags = Owned::new(raw.cast(), "Cannot copy metadata tags")?;
        let mut result = Vec::new();
        for index in 0..CFArrayGetCount(raw) {
            let tag = CFArrayGetValueAtIndex(raw, index);
            let prefix = Owned::new(CGImageMetadataTagCopyPrefix(tag), "Missing metadata prefix")?;
            let name = Owned::new(CGImageMetadataTagCopyName(tag), "Missing metadata name")?;
            let value = Owned::new(CGImageMetadataTagCopyValue(tag), "Missing metadata value")?;
            if let (Some(prefix), Some(name), Some(value)) = (
                string(prefix.ptr()),
                string(name.ptr()),
                string(value.ptr()),
            ) {
                result.push((format!("{prefix}:{name}"), value));
            }
        }
        Ok(result)
    }
}

// All dictionary accesses stay within the lifetime of the owned property tree.
unsafe fn get(dict: Ptr, key: &str) -> Ptr {
    if dict.is_null() || CFGetTypeID(dict) != CFDictionaryGetTypeID() {
        return null();
    }
    let key = CFString::new(key);
    CFDictionaryGetValue(dict as CFDictionaryRef, key.as_CFTypeRef())
}
unsafe fn number(value: Ptr) -> Option<f64> {
    if value.is_null() || CFGetTypeID(value) != CFNumberGetTypeID() {
        return None;
    }
    let mut result: f64 = 0.0;
    (CFNumberGetValue(
        value.cast(),
        kCFNumberFloat64Type,
        (&mut result as *mut f64).cast(),
    ))
    .then_some(result)
}

pub fn location(input: &Path) -> Result<(f64, f64, f64)> {
    let source = source(input)?;
    unsafe {
        let properties = Owned::new(
            CGImageSourceCopyPropertiesAtIndex(source.ptr(), 0, null()),
            "No EXIF properties found",
        )?;
        let gps = get(properties.ptr(), "{GPS}");
        let mut lat = number(get(gps, "Latitude")).context("Missing GPS latitude")?;
        let mut lon = number(get(gps, "Longitude")).context("Missing GPS longitude")?;
        if string(get(gps, "LatitudeRef")).as_deref() == Some("S") {
            lat = -lat.abs();
        }
        if string(get(gps, "LongitudeRef")).as_deref() == Some("W") {
            lon = -lon.abs();
        }
        ensure!(
            lat.is_finite() && lon.is_finite() && lat.abs() <= 90.0 && lon.abs() <= 180.0,
            "Invalid GPS coordinates"
        );
        let exif = get(properties.ptr(), "{Exif}");
        let tag_values = tags(input).unwrap_or_default();
        let creation = tag_values
            .iter()
            .find(|(k, _)| k == "xmp:CreateDate")
            .map(|(_, v)| v.clone())
            .or_else(|| string(get(exif, "DateTimeOriginal")))
            .or_else(|| string(get(exif, "DateTimeDigitized")))
            .or_else(|| string(get(get(properties.ptr(), "{TIFF}"), "DateTime")))
            .context("Missing creation date")?;
        let date = if let Some(offset) = string(get(exif, "OffsetTimeOriginal")) {
            // Prefer an explicit EXIF offset over the machine's local timezone.
            chrono::DateTime::parse_from_str(&format!("{creation}{offset}"), "%Y:%m:%d %H:%M:%S%:z")
                .map(|d| d.with_timezone(&chrono::Local))
                .or_else(|_| parse_date(&creation))?
        } else {
            parse_date(&creation)?
        };
        Ok((lat, lon, date.timestamp_millis() as f64 / 1000.0))
    }
}

/// Decode every HEIC frame and export the existing skill's inspection report and
/// primary/Light/Dark previews. `output` must be a fresh directory from the caller.
pub fn inspect(input: &Path, output: &Path) -> Result<()> {
    use crate::metadata::{decode, Sequence};
    #[link(name = "ImageIO", kind = "framework")]
    extern "C" {
        fn CGImageSourceGetType(source: Ptr) -> Ptr;
        fn CGImageSourceGetCount(source: Ptr) -> usize;
        fn CGImageSourceGetPrimaryImageIndex(source: Ptr) -> usize;
    }
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageGetWidth(image: Ptr) -> usize;
        fn CGImageGetHeight(image: Ptr) -> usize;
    }
    let source = source(input)?;
    unsafe {
        ensure!(
            string(CGImageSourceGetType(source.ptr())).as_deref() == Some("public.heic"),
            "Input is not HEIC"
        );
        let count = CGImageSourceGetCount(source.ptr());
        ensure!(count > 0, "HEIC contains no frames");
        let tags = tags(input)?;
        let payload = decode(
            &tags
                .iter()
                .find(|(key, _)| key == "apple_desktop:h24")
                .context("Missing h24 metadata")?
                .1,
        )?;
        let sequence: Sequence = plist::from_bytes(&payload)?;
        let ap = sequence.ap.context("Missing appearance metadata")?;
        let primary = CGImageSourceGetPrimaryImageIndex(source.ptr());
        ensure!(
            primary < count && ap.l < count && ap.d < count,
            "Invalid primary/Light/Dark indices"
        );
        let previews = [
            ("primary.png", primary),
            ("light.png", ap.l),
            ("dark.png", ap.d),
        ];
        let mut frames = Vec::with_capacity(count);
        let options = CFDictionary::from_CFType_pairs(&[
            (
                CFString::new("kCGImageSourceShouldCache"),
                CFBoolean::true_value(),
            ),
            (
                CFString::new("kCGImageSourceShouldCacheImmediately"),
                CFBoolean::true_value(),
            ),
        ]);
        for index in 0..count {
            let image = Owned::new(
                CGImageSourceCreateImageAtIndex(source.ptr(), index, options.as_CFTypeRef()),
                "Cannot decode HEIC frame",
            )?;
            let width = CGImageGetWidth(image.ptr());
            let height = CGImageGetHeight(image.ptr());
            let properties = Owned::new(
                CGImageSourceCopyPropertiesAtIndex(source.ptr(), index, null()),
                "Missing frame properties",
            )?;
            ensure!(
                number(get(properties.ptr(), "PixelWidth")) == Some(width as f64)
                    && number(get(properties.ptr(), "PixelHeight")) == Some(height as f64),
                "Decoded frame dimensions do not match properties"
            );
            for (name, preview_index) in previews {
                if index == preview_index {
                    let path = output.join(name);
                    ensure!(!path.exists(), "Preview already exists: {}", path.display());
                    let target = url(&path)?;
                    let kind = CFString::new("public.png");
                    let destination = Owned::new(
                        CGImageDestinationCreateWithURL(
                            target.as_CFTypeRef(),
                            kind.as_CFTypeRef(),
                            1,
                            null(),
                        ),
                        "Cannot create preview",
                    )?;
                    CGImageDestinationAddImage(destination.ptr(), image.ptr(), null());
                    ensure!(
                        CGImageDestinationFinalize(destination.ptr()),
                        "Cannot finalize preview"
                    );
                }
            }
            frames.push(serde_json::json!({"index": index, "width": width, "height": height, "decoded": true}));
        }
        // Deserialize through plist's serde support to keep integer and real values
        // as ordinary JSON numbers in the Python verifier's established contract.
        let metadata: serde_json::Value = plist::from_bytes(&payload)?;
        let report = serde_json::json!({"count": count, "primaryIndex": primary, "frames": frames,
            "metadata": metadata, "previews": previews.map(|(file, index)| serde_json::json!({"file": file, "index": index})),
            "visualReview": "not performed by this script"});
        for (name, bytes) in [
            ("metadata.plist", payload),
            ("container.json", serde_json::to_vec_pretty(&report)?),
        ] {
            use std::io::Write;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(output.join(name))?
                .write_all(&bytes)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[link(name = "CoreGraphics", kind = "framework")]
    extern "C" {
        fn CGImageGetWidth(image: Ptr) -> usize;
        fn CGImageGetHeight(image: Ptr) -> usize;
    }
    #[test]
    fn rotated_photo_decodes_upright_at_full_resolution() {
        let file = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/oriented.jpg");
        let source = source(&file).unwrap();
        unsafe {
            let image = decoded_image(&source, &file).unwrap();
            assert_eq!(
                (CGImageGetWidth(image.ptr()), CGImageGetHeight(image.ptr())),
                (32, 16)
            );
        }
    }
}
