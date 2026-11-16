#!/usr/bin/env python3
"""Copy a finished, reviewed time-based wallpaper into the GitHub Pages gallery.

Requires ImageMagick for browser previews. Original downloads are copied unchanged.
This is a delivery step, not a substitute for visual review or HEIC verification.
"""

import argparse
import hashlib
import json
from pathlib import Path
import re
import shutil
import subprocess
import sys
import tempfile
import zipfile

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT / ".agents/skills/generate-dynamic-wallpaper/scripts"))
from review_wallpaper import inspect_set
from render_gallery import read_catalog, render_page


def phase(frame):
    if frame["fileName"] == "master.png":
        hour = int(frame["time"][:2])
        return "Noon" if hour == 12 else frame["time"][:5]
    return Path(frame["fileName"]).stem.replace("-", " ").replace("_", " ").capitalize()


def add_wallpaper(manifest, heic, master, site, slug, title, description):
    if not re.fullmatch(r"[a-z0-9]+(?:-[a-z0-9]+)*", slug):
        raise ValueError("Slug must contain lowercase letters, numbers and single hyphens.")
    if not title.strip() or not description.strip():
        raise ValueError("A title and description are required.")
    magick = shutil.which("magick")
    if not magick:
        raise ValueError("Install ImageMagick (magick) to create lightweight gallery previews.")
    result = inspect_set(manifest, master)
    if heic.suffix.lower() != ".heic" or not heic.is_file() or not heic.stat().st_size:
        raise ValueError("Provide a nonempty, verified .heic package.")
    # GitHub rejects individual Git files above 100 MiB. Leave headroom.
    max_file = 95 * 1024 * 1024
    for source in [heic, *[Path(frame["path"]) for frame in result["frames"]]]:
        if source.stat().st_size > max_file:
            raise ValueError(f"{source.name} is too large for this Git-backed gallery (95 MiB limit).")
    site.mkdir(parents=True, exist_ok=True)
    catalog = read_catalog(site)
    destination = site / "wallpapers" / slug
    if destination.exists() or any(entry["id"] == slug for entry in catalog):
        raise FileExistsError(f"Wallpaper already exists: {slug}. Choose a new slug to preserve existing downloads.")
    destination.parent.mkdir(parents=True, exist_ok=True)
    prefix = f"wallpapers/{slug}"
    scene = {"id": slug, "title": title.strip(), "description": description.strip(),
             "width": result["master"]["width"], "height": result["master"]["height"], "frames": []}
    source_entries = json.loads(manifest.read_text(encoding="utf-8"))
    flags = {entry["fileName"]: entry for entry in source_entries}
    with tempfile.TemporaryDirectory(prefix=".wallpaper-", dir=destination.parent) as temporary:
        stage = Path(temporary)
        originals = stage / "originals"
        previews = stage / "previews"
        originals.mkdir()
        previews.mkdir()
        for index, frame in enumerate(sorted(result["frames"], key=lambda entry: entry["time"])):
            source = Path(frame["path"])
            name = frame["fileName"]
            shutil.copyfile(source, originals / name)
            preview = f"{index:02d}.webp"
            subprocess.run([magick, str(source), "-resize", "1600x1600>", "-quality", "85",
                            str(previews / preview)], check=True, capture_output=True, text=True)
            scene["frames"].append({"label": phase(frame), "time": frame["time"],
                                    "light": bool(flags[name].get("isForLight")),
                                    "dark": bool(flags[name].get("isForDark")),
                                    "preview": f"{prefix}/previews/{preview}",
                                    "original": {"url": f"{prefix}/originals/{name}",
                                                 "bytes": source.stat().st_size,
                                                 "sha256": frame["sha256"]}})
        package = stage / f"{slug}.heic"
        shutil.copyfile(heic, package)
        scene["heic"] = {"url": f"{prefix}/{package.name}", "bytes": package.stat().st_size,
                         "sha256": hashlib.sha256(package.read_bytes()).hexdigest()}
        sources = stage / f"{slug}-originals.zip"
        with zipfile.ZipFile(sources, "w", compression=zipfile.ZIP_DEFLATED) as archive:
            for frame in result["frames"]:
                archive.write(frame["path"], frame["fileName"])
            archive.write(manifest, "daynight.json")
        if sources.stat().st_size > max_file:
            raise ValueError("Source ZIP exceeds 95 MiB; this set needs external download hosting.")
        scene["sources"] = {"url": f"{prefix}/{sources.name}", "bytes": sources.stat().st_size}
        # Move only after every copy/preview/archive succeeds. Never copy failed attempts or review artifacts.
        catalog.append(scene)
        page = render_page(catalog)
        stage.rename(destination)
    catalog_path = site / "catalog.json"
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=site, delete=False) as stream:
            temporary_catalog = Path(stream.name)
            stream.write(json.dumps(catalog, ensure_ascii=True, indent=2) + "\n")
        (site / "index.html").write_text(page, encoding="utf-8")
        temporary_catalog.replace(catalog_path)
    except BaseException:
        shutil.rmtree(destination)
        if "temporary_catalog" in locals():
            temporary_catalog.unlink(missing_ok=True)
        raise
    return scene


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("heic", type=Path)
    parser.add_argument("--master", required=True, type=Path)
    parser.add_argument("--slug", required=True)
    parser.add_argument("--title", required=True)
    parser.add_argument("--description", required=True)
    parser.add_argument("--site", type=Path, default=ROOT / "site")
    args = parser.parse_args()
    try:
        scene = add_wallpaper(args.manifest.resolve(), args.heic.resolve(), args.master.resolve(),
                              args.site.resolve(), args.slug, args.title, args.description)
        print(f"Added {scene['title']}: {len(scene['frames'])} frames, {scene['width']}x{scene['height']}")
        print(f"Gallery: {args.site.resolve() / 'index.html'}")
        return 0
    except (OSError, ValueError, TypeError, subprocess.CalledProcessError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
