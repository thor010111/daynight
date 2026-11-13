# Review and macOS packaging

Use this repository's `daynight` command for time-based HEIC output. This skill does not modify the encoder. Read the local README/source if using a different version.

## Manifest

Save `daynight.json` beside accepted PNGs. Use one unique file per entry, one primary image, one Light image, and one Dark image. The current encoder moves the primary first, preserves the other frames' order, and maps both schedule and appearance indices to that encoded order. Keeping the primary first is convenient and also compatible with older binaries that used input-order appearance indices. Always verify the exported indices.

Use local wall-clock times with whole minutes (`HH:mm:00`); this encoder discards seconds. The list need not be chronological because each frame has its own time. The primary can also be the Light frame. For example, when the master is noon:

```json
[
  {"fileName": "master.png", "time": "12:00:00", "isPrimary": true, "isForLight": true},
  {"fileName": "night.png", "time": "00:00:00", "isForDark": true},
  {"fileName": "dawn.png", "time": "06:00:00"},
  {"fileName": "morning.png", "time": "09:00:00"},
  {"fileName": "sunset.png", "time": "18:00:00"},
  {"fileName": "blue-hour.png", "time": "20:00:00"}
]
```

Do not mix `time` with `altitude`/`azimuth`. For explicitly requested solar mode, obtain a real location, date, and calculated solar positions, and follow the repository's solar schema instead. The bundled review helper intentionally supports the time-based manifest only.

## Comparison page

Run with Python 3; no third-party Python packages are needed. Replace the example paths with actual absolute paths; resolve the script relative to the installed skill, not the current working directory.

```sh
python3 /path/to/skill/scripts/review_wallpaper.py \
  /path/to/output/daynight.json --master /path/to/output/master.png \
  --expected-size 3840x2160
```

Set `--expected-size` to the agreed output dimensions; do not simply copy an undersized master's dimensions. The helper rejects missing files, invalid/duplicate clock times, ambiguous appearance flags, reused frame paths, and different PNG dimensions. It records file hashes and creates `review.html` next to the manifest, with side-by-side thumbnails, an overlay, user-controlled blink, and native-size inspection. Open that file in a browser. It reads PNG headers, not decoded pixel content: check that every image actually renders. Its success is not a visual pass or permission to package.

Use `--check-only` to recheck without writing a page. To regenerate a page, specify a new filename using `--output /path/to/output/review-v2.html`; existing files are not overwritten. Review notes should identify inspected frame hashes so they do not silently apply to replacements.

When browser interaction is unavailable, export inspection aids using an installed ImageMagick `magick` command:

```sh
python3 /path/to/skill/scripts/review_wallpaper.py \
  /path/to/output/daynight.json --master /path/to/output/master.png \
  --expected-size 3840x2160 --output /path/to/output/review-offline.html \
  --export-overlays /path/to/output/inspection --tile-size 1024
```

Use a new inspection directory. `comparisons.json` maps every master comparison and chronological neighboring-time pair (including last-to-first) to its full-frame 50% overlay and native-size tiles. Identical pairs reuse their exported images. Tiles cover all edges, including partial tiles for odd dimensions; no resampling is performed. Source PNGs are never changed. Open each original frame, full overlay and tiles, in small batches, and record the visual result separately. Export success is not a visual pass. If ImageMagick is unavailable, use the browser or other available image-inspection tools; no global installation is required by this skill.

## Encode

Use an executable built from the current checkout, such as `target/release/daynight`. If the source has changed since the binary was built, or its provenance is uncertain, run `cargo build --release --locked`. An installed compatible `daynight` can also be used, with the primary-first manifest convention above for older versions. Building requires Rust and the native platform dependencies documented in the repository README: ImageIO on macOS, or libheif with HEVC encoding/decoding on Linux and Windows. Do not install global packages or copy binaries into system directories merely to package a wallpaper.

Run from the output folder, substituting the actual executable path:

```sh
/path/to/daynight -i daynight.json -o wallpaper.heic
/path/to/daynight -e wallpaper.heic -o metadata.plist
```

Use fresh output filenames. Input image paths are relative to the manifest; relative output paths are relative to the command's working directory. Current encoding and extraction support absolute output paths and propagate file errors. For older binaries, use a basename such as `metadata.plist` from the output folder. Confirm the new HEIC and extracted plist are nonempty, then validate the encoded content.

## Verify the package

The bundled verifier uses Python 3, the Rust `daynight-inspect` binary and the platform image backend; it needs no third-party Python packages. It runs on macOS, Linux, and Windows and reads the HEIC directly, decodes every encoded image, and compares count, dimensions, time fractions and primary/Light/Dark indices with the supplied manifest. It supports the time-based schema only.

```sh
python3 /path/to/skill/scripts/verify_wallpaper.py \
  /path/to/output/daynight.json /path/to/output/wallpaper.heic \
  --master /path/to/output/master.png --expected-size 3840x2160 \
  --output /path/to/output/heic-check
```

Use the agreed dimensions and a new output directory. The script saves `verification.json` only after all technical checks pass, plus `metadata.plist`, `container.json` and `primary.png`, `light.png`, `dark.png` for inspection. It uses the Rust `daynight-inspect` executable from `target/release/`, PATH, or `--inspector /path/to/daynight-inspect`. Open the previews to confirm their intended appearances; metadata alone does not prove their visual content. Failed runs can leave partial evidence, which is not a successful validation.

If the helper is unavailable, perform the equivalent checks manually:

Inspect `metadata.plist` with Python's standard-library `plistlib` or `plutil -p`:

- `ti` contains the expected number of schedule entries. Each `t` equals `(hour * 60 + minute) / 1440`, compared with floating-point tolerance; `i` points to the intended encoded image.
- `ap.l` and `ap.d` point to the selected Light and Dark images.
- Inspect encoded frame count and dimensions with ImageIO (`CGImageSourceGetCount`, `CGImageSourceCopyPropertiesAtIndex`) or another installed HEIC-capable reader. Decode/render the primary and Dark frame to confirm appearance assignments, not merely index ranges.

HEIC compression can change pixel values. The PNG masters remain the sources for geometry review. Successful encoding and correct metadata do not prove macOS will switch wallpapers on the user's current system; only report that behavior as tested if actually observed.

## Restricted macOS image services

In the first run, ImageIO HEIC finalization and PNG preview export failed inside the execution sandbox, then succeeded with the same inputs when macOS image-service access was available. A finalization failure does not by itself prove a sandbox problem: first check input validity and output paths. If access restrictions are indicated, use the environment's approval mechanism for a narrowly scoped retry of the same local encoder or verifier. Do not regenerate the artwork or install another encoder to work around that access failure. If the retry still fails, retain the PNGs and report the actual error; do not claim a finished package. Use a fresh verifier output directory after a failed run.

## Deliverables

Link the HEIC, the PNG/manifest directory, `review.html`, and `review.md` (which includes saved prompts). If review or encoding is incomplete, label that explicitly and omit a claim of a finished dynamic wallpaper.
