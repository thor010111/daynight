#!/usr/bin/env python3
"""Check that gallery downloads and previews exist and originals retain their hashes."""

import argparse
import hashlib
from pathlib import Path
import sys
import zipfile

from add_wallpaper import read_catalog
from render_gallery import render_page
from html.parser import HTMLParser


class PageLinks(HTMLParser):
    def __init__(self):
        super().__init__()
        self.downloads = set()
        self.images = set()
        self.targets = set()
        self.anchors = set()

    def handle_starttag(self, tag, attrs):
        attrs = dict(attrs)
        if tag == "script" or any(key.lower().startswith("on") for key in attrs):
            raise ValueError("Gallery must work as plain HTML/CSS, with no script or event handlers.")
        if "id" in attrs:
            self.targets.add(attrs["id"])
        if tag == "img":
            self.images.add(attrs["src"])
        if tag == "a":
            if "download" in attrs:
                self.downloads.add(attrs["href"])
            if attrs.get("href", "").startswith("#"):
                self.anchors.add(attrs["href"][1:])


def check(site):
    scenes = read_catalog(site)
    if not scenes:
        raise ValueError("Gallery has no wallpapers.")
    page = (site / "index.html").read_text(encoding="utf-8")
    if page != render_page(scenes):
        raise ValueError("Static gallery is out of date; run python3 scripts/render_gallery.py.")
    links = PageLinks()
    links.feed(page)
    if links.anchors - links.targets:
        raise ValueError(f"Missing anchor targets: {links.anchors - links.targets}")
    seen = set()
    for scene in scenes:
        if scene["id"] in seen:
            raise ValueError(f"Duplicate scene: {scene['id']}")
        seen.add(scene["id"])
        files = [scene["heic"], scene["sources"]]
        downloads = [scene["heic"]["url"], scene["sources"]["url"]]
        for frame in scene["frames"]:
            files.extend([frame["original"], {"url": frame["preview"]}])
            downloads.append(frame["original"]["url"])
            if frame["preview"] not in links.images:
                raise ValueError(f"Missing image in static HTML: {frame['preview']}")
        if set(downloads) - links.downloads:
            raise ValueError("Every original must have a download link in the static HTML.")
        for file in files:
            path = (site / file["url"]).resolve()
            if not path.is_relative_to(site.resolve()) or not path.is_file() or not path.stat().st_size:
                raise ValueError(f"Missing or invalid gallery file: {file['url']}")
            if "bytes" in file and path.stat().st_size != file["bytes"]:
                raise ValueError(f"Size mismatch: {file['url']}")
            if "sha256" in file and hashlib.sha256(path.read_bytes()).hexdigest() != file["sha256"]:
                raise ValueError(f"Original download changed: {file['url']}")
        with zipfile.ZipFile(site / scene["sources"]["url"]) as archive:
            if archive.testzip() is not None:
                raise ValueError(f"Corrupt source archive: {scene['id']}")
            for frame in scene["frames"]:
                name = Path(frame["original"]["url"]).name
                if hashlib.sha256(archive.read(name)).hexdigest() != frame["original"]["sha256"]:
                    raise ValueError(f"Archive image changed: {name}")
    print(f"Gallery checked: {len(scenes)} sets, {sum(len(scene['frames']) for scene in scenes)} frames. Downloads, previews and source ZIPs are intact.")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("site", nargs="?", type=Path, default=Path(__file__).resolve().parents[1] / "site")
    try:
        check(parser.parse_args().site)
    except (OSError, ValueError, KeyError, zipfile.BadZipFile) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        sys.exit(1)
