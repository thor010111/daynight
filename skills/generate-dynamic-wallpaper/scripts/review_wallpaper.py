#!/usr/bin/env python3
"""Validate a time manifest and build a read-only visual comparison page.

Standard library only. PNG header checks are not image decoding or drift detection.
"""

import argparse
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import struct
import subprocess
import sys
from urllib.parse import quote
import zlib


def inspect_png(path):
    with path.open("rb") as stream:
        header = stream.read(33)
        if (len(header) != 33 or header[:8] != b"\x89PNG\r\n\x1a\n"
                or header[8:16] != b"\x00\x00\x00\rIHDR"):
            raise ValueError(f"Not a PNG with an IHDR header: {path}")
        if zlib.crc32(header[12:29]) != struct.unpack(">I", header[29:33])[0]:
            raise ValueError(f"Invalid PNG header checksum: {path}")
        width, height = struct.unpack(">II", header[16:24])
        if not width or not height:
            raise ValueError(f"Empty PNG dimensions: {path}")
        digest = hashlib.sha256(header)
        for block in iter(lambda: stream.read(1024 * 1024), b""):
            digest.update(block)
    return {"width": width, "height": height, "sha256": digest.hexdigest()}


def inspect_set(manifest, master, expected_size=None):
    manifest, master = manifest.resolve(), master.resolve()
    entries = json.loads(manifest.read_text(encoding="utf-8"))
    if not isinstance(entries, list) or len(entries) < 2:
        raise ValueError("Manifest must contain at least two time-based frames.")
    master_info = inspect_png(master)
    if expected_size and (master_info["width"], master_info["height"]) != expected_size:
        raise ValueError(f"Master does not match requested size {expected_size[0]}x{expected_size[1]}.")
    frames = []
    seen_files, seen_times = set(), set()
    flags = {key: [] for key in ("isPrimary", "isForLight", "isForDark")}
    for index, entry in enumerate(entries):
        if not isinstance(entry, dict):
            raise ValueError(f"Entry {index} must be an object.")
        unknown = set(entry) - {"fileName", "time", *flags}
        if unknown:
            raise ValueError(f"Entry {index}: unsupported time-manifest keys {sorted(unknown)}")
        name = entry.get("fileName")
        if (not isinstance(name, str) or not name or Path(name).name != name
                or "\\" in name or Path(name).suffix.lower() != ".png"):
            raise ValueError(f"Entry {index}: fileName must be a PNG basename beside the manifest.")
        clock_time = entry.get("time")
        if not isinstance(clock_time, str) or not re.fullmatch(
                r"(?:[01][0-9]|2[0-3]):[0-5][0-9]:00", clock_time):
            raise ValueError(f"Entry {index}: time must be HH:mm:00 (whole minutes).")
        path = (manifest.parent / name).resolve()
        if path in seen_files or clock_time in seen_times:
            raise ValueError(f"Entry {index}: duplicate frame path or clock time.")
        seen_files.add(path)
        seen_times.add(clock_time)
        for flag in flags:
            if flag in entry and not isinstance(entry[flag], bool):
                raise ValueError(f"Entry {index}: {flag} must be a boolean.")
            if entry.get(flag) is True:
                flags[flag].append(index)
        info = inspect_png(path)
        if (info["width"], info["height"]) != (master_info["width"], master_info["height"]):
            raise ValueError(f"Dimension mismatch: {name} is {info['width']}x{info['height']}; "
                             f"master is {master_info['width']}x{master_info['height']}.")
        frames.append({"fileName": name, "time": clock_time, "path": str(path), **info})
    for flag, indices in flags.items():
        if len(indices) != 1:
            raise ValueError(f"Exactly one frame must set {flag} to true.")
    primary = flags["isPrimary"][0]
    encoded_indices = [primary] + [i for i in range(len(frames)) if i != primary]
    for encoded_index, input_index in enumerate(encoded_indices):
        frames[input_index]["encodedIndex"] = encoded_index
    return {"master": {"path": str(master), **master_info}, "frames": frames,
            "encodedOrder": [frames[i]["fileName"] for i in encoded_indices],
            "appearance": {"light": frames[flags["isForLight"][0]]["encodedIndex"],
                           "dark": frames[flags["isForDark"][0]]["encodedIndex"]},
            "visualReview": "not performed by this script"}


def export_overlays(result, output, tile_size):
    """Create inspection aids only. Never resize, crop or grade source frames."""
    magick = shutil.which("magick")
    if not magick:
        raise ValueError("Overlay export requires ImageMagick's magick command; HTML review does not.")
    output.mkdir(parents=True, exist_ok=False)
    master = result["master"]
    pairs = [("master", master, frame) for frame in result["frames"]
             if frame["path"] != master["path"]]
    chronological = sorted(result["frames"], key=lambda frame: frame["time"])
    pairs += [("transition", frame, chronological[(i + 1) % len(chronological)])
              for i, frame in enumerate(chronological)]
    comparisons, exported = [], {}

    def run(*args):
        subprocess.run([magick, *map(str, args)], check=True, capture_output=True, text=True)

    for kind, base, candidate in pairs:
        # A master comparison can also cover a neighboring-time transition.
        key = tuple(sorted((base["path"], candidate["path"])))
        if key not in exported:
            name = f"overlay-{len(exported):02d}"
            overlay = output / f"{name}.png"
            run(base["path"], candidate["path"], "-evaluate-sequence", "mean", overlay)
            run(overlay, "-crop", f"{tile_size}x{tile_size}", "+repage", output / f"{name}-tile-%d.png")
            exported[key] = {"overlay": overlay.name,
                             "tiles": [path.name for path in sorted(output.glob(f"{name}-tile-*.png"),
                                       key=lambda path: int(path.stem.rsplit("-", 1)[1]))]}
        comparisons.append({"kind": kind, "base": base["path"], "candidate": candidate["path"],
                            "baseSHA256": base["sha256"], "candidateSHA256": candidate["sha256"],
                            **exported[key]})
    report = {"width": master["width"], "height": master["height"], "tileSize": tile_size,
              "comparisons": comparisons, "visualReview": "not performed by this script"}
    (output / "comparisons.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    return report


PAGE = """<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width">
<title>Wallpaper comparison</title>
<style>
body{font:15px system-ui;background:#17191c;color:#eee;margin:24px}
h1{font-size:24px}label{display:inline-flex;align-items:center;gap:8px;margin:6px 16px 6px 0}
select,button{font:inherit;padding:5px}input[type=range]{width:150px}
#viewport{overflow:auto;max-height:75vh;border:1px solid #555;margin:16px 0}
#stage{position:relative;width:100%;line-height:0}#stage img{display:block;width:100%;height:auto}
#candidate{position:absolute;inset:0}#status{color:#f8cc77}#gallery{display:grid;grid-template-columns:repeat(auto-fit,minmax(240px,1fr));gap:16px}
figure{margin:0}figure img{width:100%;display:block}figcaption{margin:6px 0 18px}
code{overflow-wrap:anywhere}details{margin:20px 0}
</style>
<h1>Wallpaper comparison</h1>
<p>Compare silhouettes, object counts, textures, crop and perspective. Lighting differences are expected.
No automatic visual pass is assigned. Check all frames and the last-to-first transition.</p>
<div id="status">Loading images…</div>
<label>Base <select id="baseSelect"></select></label>
<label>Compare <select id="compareSelect"></select></label>
<label>Overlay <input id="opacity" type="range" min="0" max="1" step="0.01" value="0.5"></label>
<label><input type="checkbox" id="blink"> Blink</label>
<label><input type="checkbox" id="native"> 100% size</label>
<div id="viewport"><div id="stage"><img id="base" alt="Base frame"><img id="candidate" alt="Comparison frame"></div></div>
<p>100% uses one image pixel per CSS pixel; scroll to inspect corners. Blink is off by default.</p>
<div id="gallery"></div><details><summary>Inspected file hashes and dimensions</summary><pre><code id="report"></code></pre></details>
<script>
const data = __DATA__;
const byId = id => document.getElementById(id);
const items = [{...data.master,label:'Original master'}, ...data.frames.map(f => ({...f,label:f.time+' — '+f.fileName}))];
let timer = null, showing = true, loaded = 0;
const failed = new Set();
function status() {
  byId('status').textContent = failed.size ? 'Images failed to decode/load: '+[...failed].join(', ') :
    loaded+' / '+items.length+' images loaded. Visual review still required.';
}
for (const [index,item] of items.entries()) {
  for (const select of [byId('baseSelect'),byId('compareSelect')]) {
    const option = document.createElement('option'); option.value=index; option.textContent=item.label; select.append(option);
  }
  const figure=document.createElement('figure'), img=document.createElement('img'), caption=document.createElement('figcaption');
  img.alt=item.label; img.onload=()=>{loaded++;status();}; img.onerror=()=>{failed.add(item.label);status();}; img.src=item.url;
  caption.textContent=item.label; figure.append(img,caption); byId('gallery').append(figure);
}
byId('compareSelect').value = items.length > 2 ? 2 : 1;
function opacity() { byId('candidate').style.opacity=byId('blink').checked ? (showing ? '1':'0') : byId('opacity').value; }
function render() {
  byId('base').src=items[Number(byId('baseSelect').value)].url;
  byId('candidate').src=items[Number(byId('compareSelect').value)].url;
  byId('stage').style.width=byId('native').checked ? data.master.width+'px' : '100%'; opacity();
}
for (const id of ['baseSelect','compareSelect','native']) byId(id).addEventListener('change',render);
byId('opacity').addEventListener('input',opacity);
byId('blink').addEventListener('change',()=>{
  clearInterval(timer); showing=true;
  if(byId('blink').checked) timer=setInterval(()=>{showing=!showing;opacity();},700);
  opacity();
});
byId('report').textContent=JSON.stringify(data,null,2); render();
</script></html>
"""


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("--master", required=True, type=Path)
    parser.add_argument("--output", type=Path, help="New HTML path; defaults to review.html beside manifest")
    parser.add_argument("--check-only", action="store_true")
    parser.add_argument("--expected-size", help="Required output dimensions, e.g. 3840x2160")
    parser.add_argument("--export-overlays", type=Path, metavar="NEW_DIRECTORY",
                        help="Export full-frame overlays, native-size tiles and neighboring-time comparisons (requires magick)")
    parser.add_argument("--tile-size", type=int, default=1024, help="Maximum native-size tile edge in pixels (default: 1024)")
    args = parser.parse_args()
    if args.tile_size < 1:
        parser.error("--tile-size must be positive")
    if args.check_only and args.export_overlays:
        parser.error("--check-only cannot be combined with --export-overlays")
    try:
        manifest, master = args.manifest.resolve(), args.master.resolve()
        expected_size = None
        if args.expected_size:
            if not re.fullmatch(r"[1-9][0-9]*x[1-9][0-9]*", args.expected_size):
                raise ValueError("--expected-size must be WIDTHxHEIGHT, e.g. 3840x2160.")
            expected_size = tuple(int(value) for value in args.expected_size.split("x"))
        result = inspect_set(manifest, master, expected_size)
        if not args.check_only:
            output = (args.output or manifest.parent / "review.html").resolve()
            if output.exists():
                raise FileExistsError(f"Comparison page already exists: {output}; choose a new --output path.")
            if args.export_overlays:
                overlay_dir = args.export_overlays.resolve()
                export_overlays(result, overlay_dir, args.tile_size)
                print(f"Inspection aids: {overlay_dir}")
            for item in [result["master"], *result["frames"]]:
                item["url"] = quote(os.path.relpath(item["path"], output.parent), safe="/")
            payload = json.dumps(result, ensure_ascii=True).replace("<", "\\u003c").replace(">", "\\u003e").replace("&", "\\u0026")
            with output.open("x", encoding="utf-8") as stream:
                stream.write(PAGE.replace("__DATA__", payload))
            print(f"Comparison page: {output}")
        print(json.dumps(result, indent=2))
        print("Manifest and PNG headers checked. Decoding and visual consistency still require inspection.")
        return 0
    except subprocess.CalledProcessError as error:
        print(f"ERROR: ImageMagick failed; any partial inspection output is incomplete. {error.stderr.strip()}", file=sys.stderr)
        return 1
    except (OSError, ValueError, TypeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
