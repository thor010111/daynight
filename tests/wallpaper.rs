use daynight::metadata::{time_fraction, Appearance, PictureInfo, Plan, Sequence};
use std::{
    path::{Path, PathBuf},
    sync::atomic::{AtomicUsize, Ordering},
};

struct Dir(PathBuf);
impl Dir {
    fn new() -> Self {
        static NEXT: AtomicUsize = AtomicUsize::new(0);
        let dir = std::env::temp_dir().join(format!(
            "daynight-test-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        std::fs::create_dir(&dir).unwrap();
        Self(dir)
    }
}
impl Drop for Dir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
fn pictures(mode: &str) -> Vec<PictureInfo> {
    let mut values = serde_json::json!([
        {"fileName":"dawn.png"},
        {"fileName":"night.png", "isForDark":true},
        {"fileName":"noon.png", "isPrimary":true,"isForLight":true},
        {"fileName":"night.png"}
    ]);
    for (i, value) in values.as_array_mut().unwrap().iter_mut().enumerate() {
        if mode == "h24" {
            value["time"] = serde_json::json!(["06:00:00", "00:00:00", "12:00:00", "20:00:00"][i]);
        }
        if mode == "solar" {
            value["altitude"] = serde_json::json!(i);
            value["azimuth"] = serde_json::json!(i * 90);
        }
    }
    serde_json::from_value(values).unwrap()
}
#[test]
fn reordered_and_duplicate_images_keep_schedule_and_appearance_indices() {
    for mode in ["h24", "solar", "apr"] {
        let pictures = pictures(mode);
        let plan = Plan::new(&pictures).unwrap();
        assert_eq!(plan.images, ["noon.png", "dawn.png", "night.png"]);
        assert_eq!(plan.key, mode);
        assert!(plan.payload.starts_with(b"bplist00"));
        let ap = if mode == "apr" {
            plist::from_bytes::<Appearance>(&plan.payload).unwrap()
        } else {
            let seq: Sequence = plist::from_bytes(&plan.payload).unwrap();
            if mode == "h24" {
                let items = seq.ti.unwrap();
                assert_eq!(items.iter().map(|i| i.i).collect::<Vec<_>>(), [1, 2, 0, 2]);
                assert_eq!(
                    items.iter().map(|i| i.t).collect::<Vec<_>>(),
                    [0.25, 0.0, 0.5, 20.0 / 24.0]
                );
            } else {
                assert_eq!(
                    seq.si.unwrap().iter().map(|i| i.i).collect::<Vec<_>>(),
                    [1, 2, 0, 2]
                );
            }
            seq.ap.unwrap()
        };
        assert_eq!(ap, Appearance { l: 0, d: 2 });
    }
}
#[test]
fn times_and_invalid_manifests() {
    assert_eq!(
        time_fraction("14:32:59").unwrap(),
        14.0 / 24.0 + 32.0 / 1440.0
    );
    assert_eq!(
        time_fraction("2026-11-13T14:32:00").unwrap(),
        time_fraction("14:32:00").unwrap()
    );
    assert!(time_fraction("24:00:00").is_err());
    assert!(time_fraction("nonsense").is_err());
    assert!(Plan::new(&[]).is_err());
    let mut p = pictures("h24");
    p[0].time = Some("invalid".into());
    assert!(Plan::new(&p).is_err());
}
#[test]
fn sun_matches_suncalc_reference() {
    // SunCalc's published example: 2013-03-05 00:00 UTC at 50.5 N, 30.5 E.
    let time = chrono::DateTime::parse_from_rfc3339("2013-03-05T00:00:00Z").unwrap();
    let pos = daynight::sun::position(50.5, 30.5, time.timestamp() as f64);
    assert!((pos.altitude.to_radians() - -0.7000406838781611).abs() < 1e-10);
    assert!(((pos.azimuth - 180.0).to_radians() - -2.5003175907168385).abs() < 1e-10);
}
#[test]
fn missing_and_invalid_images_preserve_existing_output() {
    let dir = Dir::new();
    let output = dir.0.join("out.heic");
    std::fs::write(&output, b"existing output").unwrap();
    let p = pictures("h24");
    let plan = Plan::new(&p).unwrap();
    assert!(daynight::native::generate(&plan, &dir.0, &output).is_err());
    for name in &plan.images {
        std::fs::write(dir.0.join(name), b"invalid image").unwrap();
    }
    assert!(daynight::native::generate(&plan, &dir.0, &output).is_err());
    assert_eq!(std::fs::read(output).unwrap(), b"existing output");
    assert_eq!(
        std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>()
            .len(),
        4,
        "{:?}",
        std::fs::read_dir(&dir.0)
            .unwrap()
            .map(|e| e.unwrap().file_name())
            .collect::<Vec<_>>()
    );
}
#[test]
fn native_heic_roundtrip_all_modes_and_extraction_failures() {
    let dir = Dir::new();
    for name in ["noon.png", "dawn.png", "night.png"] {
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frame.png"),
            dir.0.join(name),
        )
        .unwrap();
    }
    for mode in ["h24", "solar", "apr"] {
        let p = pictures(mode);
        let plan = Plan::new(&p).unwrap();
        let output = dir.0.join(format!("{mode}.heic"));
        std::fs::write(&output, b"old destination").unwrap();
        daynight::native::generate(&plan, &dir.0, &output).unwrap();
        if mode == "h24" {
            let inspection = dir.0.join("inspection");
            std::fs::create_dir(&inspection).unwrap();
            let status = std::process::Command::new(env!("CARGO_BIN_EXE_daynight-inspect"))
                .arg(&output)
                .arg(&inspection)
                .status()
                .unwrap();
            assert!(status.success());
            let report: serde_json::Value =
                serde_json::from_slice(&std::fs::read(inspection.join("container.json")).unwrap())
                    .unwrap();
            assert_eq!(report["count"], 3);
            assert_eq!(report["primaryIndex"], 0);
            assert_eq!(report["metadata"]["ap"]["d"], 2);
            assert_eq!(report["metadata"]["ti"].as_array().unwrap().len(), 4);
            assert_eq!(report["frames"][0]["width"], 32);
            for file in ["primary.png", "light.png", "dark.png"] {
                assert!(inspection.join(file).is_file());
            }
        }
        let tags = daynight::native::tags(&output).unwrap();
        let value = &tags
            .iter()
            .find(|(k, _)| k == &format!("apple_desktop:{mode}"))
            .unwrap()
            .1;
        assert_eq!(daynight::metadata::decode(value).unwrap(), plan.payload);
        let plist = dir.0.join(format!("{mode}.plist"));
        daynight::extract(&output, Some(&plist)).unwrap();
        assert_eq!(std::fs::read(plist).unwrap(), plan.payload);
        assert!(daynight::extract(&output, Some(&dir.0.join("missing/output.plist"))).is_err());
    }
}
#[test]
fn cli_rejects_missing_values_and_unknown_options() {
    for args in [
        vec!["-i"],
        vec!["-o", "-i", "file"],
        vec!["--unknown"],
        vec!["-i", "a", "-e", "b"],
    ] {
        assert!(!std::process::Command::new(env!("CARGO_BIN_EXE_daynight"))
            .args(args)
            .output()
            .unwrap()
            .status
            .success());
    }
    for binary in [
        env!("CARGO_BIN_EXE_daynight"),
        env!("CARGO_BIN_EXE_daynight-exif"),
    ] {
        assert!(std::process::Command::new(binary)
            .arg("-h")
            .output()
            .unwrap()
            .status
            .success());
        assert!(std::process::Command::new(binary)
            .arg("-v")
            .output()
            .unwrap()
            .status
            .success());
    }
}

#[test]
fn exif_preserves_fractional_coordinates_hemispheres_and_timezone() {
    for (name, sign) in [("north-east.png", 1.0), ("south-west.png", -1.0)] {
        let path = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("tests/fixtures")
            .join(name);
        let (lat, lon, seconds) = daynight::native::location(&path).unwrap();
        assert!((lat - 50.5 * sign).abs() < 1e-9);
        assert!((lon - 30.5 * sign).abs() < 1e-9);
        assert_eq!(seconds, 1362441600.0);
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_daynight-exif"))
            .arg(path)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        let text = String::from_utf8(result.stdout).unwrap();
        let json: serde_json::Value =
            serde_json::from_str(text.strip_prefix("Output:\n").unwrap()).unwrap();
        assert!(json[0]["azimuth"].is_number());
    }
    assert!(daynight::native::location(
        &Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frame.png")
    )
    .is_err());
}

// Exercise both independently implemented backends in each direction. In
// particular, ImageIO must see the same frame indices and plist in libheif files.
#[cfg(all(target_os = "macos", feature = "portable"))]
#[test]
fn libheif_and_imageio_exchange_wallpapers_without_metadata_or_frame_order_drift() {
    use daynight::native::{macos, portable};
    let dir = Dir::new();
    for (name, color) in [
        ("dawn.png", [255, 0, 0, 255]),
        ("night.png", [0, 0, 255, 255]),
        ("noon.png", [0, 255, 0, 255]),
    ] {
        image::RgbaImage::from_pixel(256, 256, image::Rgba(color))
            .save(dir.0.join(name))
            .unwrap();
    }
    for mode in ["h24", "solar", "apr"] {
        let p = pictures(mode);
        let plan = Plan::new(&p).unwrap();
        for (name, generate, tags, inspect) in [
            (
                "portable",
                portable::generate as fn(&Plan<'_>, &Path, &Path) -> anyhow::Result<()>,
                macos::tags as fn(&Path) -> anyhow::Result<Vec<(String, String)>>,
                macos::inspect as fn(&Path, &Path) -> anyhow::Result<()>,
            ),
            (
                "imageio",
                macos::generate,
                portable::tags,
                portable::inspect,
            ),
        ] {
            let output = dir.0.join(format!("{name}-{mode}.heic"));
            // Both backends must replace an existing destination on success.
            std::fs::write(&output, b"old file").unwrap();
            generate(&plan, &dir.0, &output).unwrap();
            let metadata = tags(&output).unwrap();
            let encoded = metadata
                .iter()
                .find(|(k, _)| k == &format!("apple_desktop:{mode}"))
                .unwrap();
            assert_eq!(
                daynight::metadata::decode(&encoded.1).unwrap(),
                plan.payload
            );
            if mode == "h24" {
                let evidence = dir.0.join(name);
                std::fs::create_dir(&evidence).unwrap();
                inspect(&output, &evidence).unwrap();
                let report: serde_json::Value = serde_json::from_slice(
                    &std::fs::read(evidence.join("container.json")).unwrap(),
                )
                .unwrap();
                assert_eq!(report["count"], 3);
                assert_eq!(report["primaryIndex"], 0);
                assert_eq!(report["metadata"]["ap"]["d"], 2);
                for (preview, channel) in [("primary.png", 1), ("light.png", 1), ("dark.png", 2)] {
                    let pixel = image::open(evidence.join(preview))
                        .unwrap()
                        .to_rgb8()
                        .get_pixel(16, 16)
                        .0;
                    assert!(pixel[channel] > 240, "{name}/{preview}: {pixel:?}");
                    for (index, value) in pixel.into_iter().enumerate() {
                        if index != channel {
                            assert!(value < 15);
                        }
                    }
                }
            }
        }
    }
}

#[test]
fn cli_generates_and_extracts_with_unicode_paths() {
    let root = Dir::new();
    let directory = root.0.join("Grüße 日本語");
    std::fs::create_dir(&directory).unwrap();
    for name in ["täg.png", "nücht.png"] {
        std::fs::copy(
            Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/frame.png"),
            directory.join(name),
        )
        .unwrap();
    }
    let manifest = directory.join("zeit.json");
    std::fs::write(
        &manifest,
        serde_json::to_vec(&serde_json::json!([
            {"fileName": "täg.png", "isPrimary": true, "isForLight": true, "time": "12:00:00"},
            {"fileName": "nücht.png", "isForDark": true, "time": "00:00:00"}
        ]))
        .unwrap(),
    )
    .unwrap();
    let output = directory.join("nächt.heic");
    let plist = directory.join("métadata.plist");
    for args in [
        vec![
            std::ffi::OsStr::new("-i"),
            manifest.as_os_str(),
            std::ffi::OsStr::new("-o"),
            output.as_os_str(),
        ],
        vec![
            std::ffi::OsStr::new("-e"),
            output.as_os_str(),
            std::ffi::OsStr::new("-o"),
            plist.as_os_str(),
        ],
    ] {
        let result = std::process::Command::new(env!("CARGO_BIN_EXE_daynight"))
            .args(args)
            .output()
            .unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
    }
    let schedule: Sequence = plist::from_bytes(&std::fs::read(plist).unwrap()).unwrap();
    assert_eq!(schedule.ap.unwrap(), Appearance { l: 0, d: 1 });
}
