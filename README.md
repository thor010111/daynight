# daynight

Create dynamic macOS wallpapers from your own images. Written in Rust, daynight
packages images into a HEIC file with Apple's desktop metadata, so the
wallpaper can change with the time of day, the sun's position, or Light/Dark appearance.

The project includes three command-line tools:

- **daynight** creates wallpapers and extracts their metadata.
- **daynight-exif** calculates solar altitude and azimuth from photos with GPS and capture-time metadata.
- **daynight-inspect** checks time-based HEIC files and exports preview images.

## Requirements

- macOS: HEIC encoding uses Apple's system ImageIO framework.
- Rust 1.85 or newer and Cargo.
- Xcode Command Line Tools for the macOS SDK and linker.

No Swift compiler or third-party image codec is required.

## Build

```sh
cargo build --release --locked
```

The executables are in `target/release/`. Use them directly or copy them into a
directory on your `PATH`.

## Create your first wallpaper

Place your images and a `daynight.json` file in the same directory. For a
time-based wallpaper, use a manifest like this:

```json
[
  {
    "fileName": "day.png",
    "isPrimary": true,
    "isForLight": true,
    "time": "08:00:00"
  },
  {
    "fileName": "night.png",
    "isForDark": true,
    "time": "20:00:00"
  }
]
```

Then run:

```sh
target/release/daynight -i /path/to/daynight.json -o wallpaper.heic
```

Select the generated HEIC in macOS Wallpaper settings. The command creates the
file; it does not change your desktop wallpaper automatically.

Image paths are relative to the manifest directory. Output paths are relative to
your current working directory, unless absolute. The default output is `output.heic`.

## Wallpaper modes

Each manifest is a JSON array. Every entry has a `fileName` and may set these flags:

| Field | Purpose |
| --- | --- |
| `isPrimary` | Selects the initial preview image. |
| `isForLight` | Selects the image for Light appearance. |
| `isForDark` | Selects the image for Dark appearance. |

Use one primary, one Light, and one Dark selection. The primary image is encoded
first; other images retain their relative order. Repeated filenames share one
encoded image, while retaining their separate schedule entries.

**Time-based:** add a `time` such as `"08:00:00"` to each entry. Full timestamps such
as `"2026-11-13T08:00:00+01:00"` are also accepted. Explicit timezone offsets are
converted to the system timezone. Schedule fractions use hours and minutes;
seconds are ignored.

**Solar:** add numeric `altitude` and `azimuth` values, in degrees, to each entry:

```json
[
  { "fileName": "day.png", "isPrimary": true, "isForLight": true, "altitude": 28.0, "azimuth": 180.0 },
  { "fileName": "night.png", "isForDark": true, "altitude": -20.0, "azimuth": 340.0 }
]
```

**Appearance:** omit time and solar coordinates and supply Light and Dark images:

```json
[
  { "fileName": "day.png", "isPrimary": true, "isForLight": true },
  { "fileName": "night.png", "isForDark": true }
]
```

Use a consistent mode throughout a manifest. Solar metadata takes precedence if
both solar coordinates and times are supplied.

## Extract metadata and calculate solar positions

```sh
# Print wallpaper metadata.
target/release/daynight -e wallpaper.heic

# Also save the embedded binary property list.
target/release/daynight -e wallpaper.heic -o metadata.plist

# Calculate solar coordinates from geotagged photos.
target/release/daynight-exif photo1.jpg photo2.jpg
```

The EXIF command prints JSON preceded by an `Output:` heading. Photos must contain
GPS coordinates and a creation timestamp. Capture times without timezone offsets
are interpreted in the system timezone.

Run `daynight --help` or `daynight-exif --help` for command options.

## Verify a wallpaper

The bundled Python wrapper checks a time-based wallpaper against its manifest,
including frame count, dimensions, schedule, and primary/Light/Dark indices:

```sh
python3 skills/generate-dynamic-wallpaper/scripts/verify_wallpaper.py \
  /path/to/daynight.json /path/to/wallpaper.heic \
  --master /path/to/day.png --expected-size 3840x2160 \
  --output /path/to/new-verification-directory
```

It uses the release build of `daynight-inspect` and exports previews for visual
review. The verification directory must be new. See the
[wallpaper workflow](skills/generate-dynamic-wallpaper/references/daynight.md)
for details. Desktop switching still needs to be checked in macOS.

## Development

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
python3 -m unittest discover -s skills/generate-dynamic-wallpaper/tests -v
```

Native HEIC tests require access to macOS image encoding services, which may be
restricted inside sandboxes. The encoder avoids intermediate TIFF conversion,
uses a hash map for image indices, and writes into a temporary directory before
replacing the destination after successful encoding.

## Credits and license

Inspired by [Marcin Czachurski's wallpapper](https://github.com/mczachurski/wallpapper).

Solar calculations use equations from
[SunCalc by Vladimir Agafonkin](https://github.com/mourner/suncalc).

Released under the [MIT License](LICENSE.md).
