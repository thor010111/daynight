# daynight

Create dynamic macOS wallpapers on **macOS, Linux, or Windows**. Written in Rust,
daynight packages images into a HEIC file with Apple's desktop metadata, so the
wallpaper can change with the time of day, the sun's position, or Light/Dark appearance.
All three platforms can create and inspect these files. Automatic desktop switching
on Linux and Windows is outside the scope of these tools.

The project includes three command-line tools:

- **daynight** creates wallpapers and extracts their metadata.
- **daynight-exif** calculates solar altitude and azimuth from photos with GPS and capture-time metadata.
- **daynight-inspect** checks time-based HEIC files and exports preview images.

## Requirements

Rust 1.85 or newer and Cargo are required on every platform.

### macOS

Install Xcode Command Line Tools for the macOS SDK and linker. The default build
uses Apple's ImageIO framework and requires no third-party image codec.

### Linux

Install `pkg-config`, libheif **1.17 or newer**, an HEVC encoder (such as x265), and
an HEVC decoder (such as libde265). On Ubuntu 24.04:

```sh
sudo apt-get update
sudo apt-get install pkg-config libheif-dev libheif-plugin-x265 libheif-plugin-libde265
```

Package names differ across distributions. A libheif build with AVIF support alone
cannot create HEIC wallpapers; the HEVC encoder must be installed and discoverable.

### Windows

Use the MSVC Rust toolchain with Visual Studio C++ Build Tools and
[vcpkg](https://github.com/microsoft/vcpkg). From Developer PowerShell, with
`VCPKG_ROOT` pointing to your vcpkg installation:

```powershell
& "$env:VCPKG_ROOT/vcpkg.exe" install "libheif[core,hevc,libde265]:x64-windows"
$env:VCPKGRS_TRIPLET = "x64-windows"
$env:VCPKGRS_DYNAMIC = "1"
$env:PATH = "$env:VCPKG_ROOT/installed/x64-windows/bin;$env:PATH"
# For vcpkg versions that build codecs as separate plugins:
if (Test-Path "$env:VCPKG_ROOT/installed/x64-windows/plugins/libheif") {
  $env:LIBHEIF_PLUGIN_PATH = "$env:VCPKG_ROOT/installed/x64-windows/plugins/libheif"
}
cargo build --release --locked
```

Keep libheif and its codec DLLs on `PATH` or beside the executables when running
them. The Windows CI artifact includes these DLLs, codec plugins when present,
and their license notices. Keep its `plugins/` directory beside the executables.
Native libraries and codecs retain their own licenses, separate from this
repository's MIT license.

## Build

```sh
cargo build --release --locked
```

The executables are in `target/release/`. Use them directly or copy them into a
directory on your `PATH`. Windows executables have the `.exe` suffix.
CI produces Linux and Windows artifacts and universal macOS binaries.

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

## Generate native 4K wallpaper frames

The [wallpaper workflow](.agents/skills/generate-dynamic-wallpaper/SKILL.md) uses
`sourceful/riverflow-v2.6-pro` through OpenRouter for the master and every relighting
edit. It requests native 3840 × 2160 PNGs by default, checks the returned dimensions,
and requires visual review before HEIC packaging. Smaller images fail the native
resolution requirement; this workflow does not upscale them.

Keep `OPENROUTER_API_KEY` in the repository-root `.env` or export it in the environment. See the
[OpenRouter reference](.agents/skills/generate-dynamic-wallpaper/references/openrouter.md)
for exact-size requests, original-master references, response decoding, and the
4.5 MB request-size limit.

## Verify a wallpaper

The bundled Python wrapper checks a time-based wallpaper against its manifest,
including frame count, dimensions, schedule, and primary/Light/Dark indices:

```sh
python3 .agents/skills/generate-dynamic-wallpaper/scripts/verify_wallpaper.py \
  /path/to/daynight.json /path/to/wallpaper.heic \
  --master /path/to/day.png --expected-size 3840x2160 \
  --output /path/to/new-verification-directory
```

It uses the release build of `daynight-inspect` and exports previews for visual
review. The verification directory must be new. See the
[wallpaper workflow](.agents/skills/generate-dynamic-wallpaper/references/daynight.md)
for details. Desktop switching still needs to be checked in macOS.

## Wallpaper gallery

The download gallery lives in [`site/`](site/index.html). It is plain HTML and CSS,
with no browser JavaScript. CSS provides System/Day/Night themes and matching
featured artwork. Time links jump to the individual frames. Every image links to
its full-resolution original and has a PNG download; each collection includes a
dynamic macOS HEIC and an originals ZIP.

After generating, reviewing and verifying a set, copy it into the gallery:

```sh
python3 scripts/add_wallpaper.py \
  output/wallpapers/my-scene/daynight.json \
  output/wallpapers/my-scene/my-scene.heic \
  --master output/wallpapers/my-scene/master.png \
  --slug my-scene --title "My scene" \
  --description "A short description of the scene."
python3 scripts/check_gallery.py
python3 -m http.server 4173 --directory site
```

Open `http://localhost:4173`. Adding a set requires ImageMagick's `magick` for
lightweight WebP previews. Original PNG and HEIC files are copied unchanged.
The helper updates `site/catalog.json` and renders `site/index.html` automatically.
To change the page layout, edit `scripts/templates/gallery.html` and regenerate
with `python3 scripts/render_gallery.py`. CSS can be edited directly in `site/`.
Existing slugs are preserved; use a new slug for a new version. The helper copies
only manifest frames and the final HEIC, leaving local review and failed attempts
in the ignored `output/` folder. Its checks do not replace visual/HEIC verification.

To publish, select **Settings → Pages → Build and deployment → Source → GitHub
Actions** in this repository. The [Pages workflow](.github/workflows/pages.yml)
checks the downloads and deploys `site/` on pushes to `main`, or through a manual
workflow run on `main`. Pull requests only run checks. The expected project URL is
`https://thor010111.github.io/daynight/`; it is available only after a successful
deployment. The workflow follows GitHub's
[custom Pages workflow](https://docs.github.com/en/pages/getting-started-with-github-pages/using-custom-workflows-with-github-pages).

Keep the tracked gallery below GitHub Pages' 1 GB site limit. The helper rejects
individual files and source ZIPs over 95 MiB to stay below GitHub's Git file limit;
larger collections need separate download hosting.

Use `wallpaper: add <scene>` for wallpaper-only commits, for example:

```sh
git add site/
git commit -m "wallpaper: add serene desert oasis"
```

The Build workflow skips pushes and pull requests whose changes are entirely
inside `site/`. Gallery checks and deployment still run as described above.
The changed paths control this behavior; the commit message is a naming convention.
Changes outside `site/`, including code, scripts, skills and workflows, run the
Build workflow as usual. GitHub's global `[skip ci]` marker also skips the gallery
workflow, so omit it when publishing wallpapers.

## Development

```sh
cargo test --locked
cargo fmt --check
cargo clippy --locked --all-targets -- -D warnings
python3 -m unittest discover -s .agents/skills/generate-dynamic-wallpaper/tests -v
python3 -m unittest discover -s scripts/tests -v
```

The default backend is ImageIO on macOS and libheif on Linux/Windows. The portable
backend accepts PNG, JPEG, TIFF, WebP, BMP, and libheif-readable HEIC/HEIF inputs,
normalizes photo orientation, preserves embedded ICC profiles, and decodes previews.
It reads GPS coordinates and capture times from EXIF in JPEG, PNG, TIFF, WebP, and
HEIF files. The CLI and Apple `solar`, `h24`, and `apr` plist formats are shared.

On macOS, install libheif to run the portable backend and interoperability tests:

```sh
brew install libheif
cargo test --locked --all-features
```

`--features portable` selects libheif on macOS too; Linux and Windows select it
automatically. Interoperability tests check all three wallpaper modes in both
directions between ImageIO and libheif, including frame order and preview colors.
Native ImageIO tests need macOS image-service access, which sandboxes may restrict.

Images are indexed through a hash map and decoded one at a time. Outputs replace
the destination only after successful encoding. On Unix, libheif writes directly
to a temporary file. On Windows, its encoded output is buffered and written through
Rust file IO so Unicode paths work. Neither backend changes desktop settings.

## Credits and license

Inspired by [Marcin Czachurski's wallpapper](https://github.com/mczachurski/wallpapper).

Solar calculations use equations from
[SunCalc by Vladimir Agafonkin](https://github.com/mourner/suncalc).

Released under the [MIT License](LICENSE.md).
