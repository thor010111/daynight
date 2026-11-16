#!/usr/bin/env python3
"""Verify a time-based HEIC against its PNG manifest using daynight-inspect.

Exports primary/Light/Dark previews for manual inspection. Does not certify
visual fidelity or test desktop switching. Requires the Rust daynight-inspect executable.
"""

import argparse
import hashlib
import json
import math
from pathlib import Path
import re
import shutil
import subprocess
import sys

from review_wallpaper import inspect_set


def validate_container(expected, actual):
    frames = expected["frames"]
    if actual.get("count") != len(frames) or actual.get("primaryIndex") != 0:
        raise ValueError("Encoded image count or primary index does not match the manifest.")
    encoded = actual.get("frames", [])
    if len(encoded) != len(frames):
        raise ValueError("Missing decoded image results.")
    for index, frame in enumerate(encoded):
        if (frame.get("index") != index or frame.get("decoded") is not True
                or (frame.get("width"), frame.get("height")) !=
                (expected["master"]["width"], expected["master"]["height"])):
            raise ValueError(f"Encoded image {index} failed decoding or has incorrect dimensions.")
    metadata = actual.get("metadata", {})
    appearance = metadata.get("ap", {})
    if any(type(appearance.get(key)) is not int or appearance[key] != expected["appearance"][name]
           for key, name in (("l", "light"), ("d", "dark"))):
        raise ValueError("Encoded Light/Dark assignments do not match the manifest.")
    times = metadata.get("ti", [])
    if len(times) != len(frames):
        raise ValueError("Encoded schedule length does not match the manifest.")
    by_index = {}
    for item in times:
        index, time = item.get("i"), item.get("t")
        if (type(index) is not int or index in by_index or not 0 <= index < len(frames)
                or type(time) not in (int, float) or not math.isfinite(time)):
            raise ValueError("Invalid or duplicate encoded schedule entry.")
        by_index[index] = time
    for frame in frames:
        hour, minute, _ = map(int, frame["time"].split(":"))
        if not math.isclose(by_index[frame["encodedIndex"]], (hour * 60 + minute) / 1440,
                            rel_tol=0, abs_tol=1e-12):
            raise ValueError(f"Encoded schedule does not match {frame['fileName']} at {frame['time']}.")


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("manifest", type=Path)
    parser.add_argument("heic", type=Path)
    parser.add_argument("--master", required=True, type=Path)
    parser.add_argument("--expected-size", required=True, help="Agreed dimensions, e.g. 3840x2160")
    parser.add_argument("--output", required=True, type=Path, help="New directory for validation evidence and previews")
    parser.add_argument("--inspector", type=Path, help="Path to the Rust daynight-inspect executable")
    args = parser.parse_args()
    try:
        if not re.fullmatch(r"[1-9][0-9]*x[1-9][0-9]*", args.expected_size):
            raise ValueError("--expected-size must be WIDTHxHEIGHT.")
        expected = inspect_set(args.manifest.resolve(), args.master.resolve(),
                               tuple(map(int, args.expected_size.split("x"))))
        heic = args.heic.resolve()
        if not heic.is_file() or heic.stat().st_size == 0:
            raise ValueError("HEIC is missing or empty.")
        executable = "daynight-inspect.exe" if sys.platform == "win32" else "daynight-inspect"
        bundled = Path(__file__).resolve().parents[4] / "target" / "release" / executable
        inspector = args.inspector or (bundled if bundled.is_file() else shutil.which("daynight-inspect"))
        if not inspector:
            raise ValueError("HEIC verification requires daynight-inspect (cargo build --release --locked).")
        output = args.output.resolve()
        output.mkdir(parents=True, exist_ok=False)
        subprocess.run([str(inspector), str(heic), str(output)],
                       check=True, capture_output=True, text=True)
        actual = json.loads((output / "container.json").read_text(encoding="utf-8"))
        validate_container(expected, actual)
        report = {"heic": str(heic), "heicSHA256": hashlib.sha256(heic.read_bytes()).hexdigest(),
                  "manifest": expected, "container": actual, "technicalValidation": "pass",
                  "visualReview": "not performed by this script", "desktopSwitching": "not tested"}
        (output / "verification.json").write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
        print(f"Verified {len(expected['frames'])} images at {args.expected_size}, schedule and appearance indices.")
        print(f"Evidence: {output / 'verification.json'}")
        print("Open primary.png, light.png and dark.png to confirm their visual assignments. Desktop switching was not tested.")
        return 0
    except subprocess.CalledProcessError as error:
        print(f"ERROR: Native inspection failed; partial output is incomplete. {error.stderr.strip()}", file=sys.stderr)
        return 1
    except (OSError, ValueError, TypeError, KeyError, AttributeError) as error:
        print(f"ERROR: {error}", file=sys.stderr)
        return 1


if __name__ == "__main__":
    sys.exit(main())
