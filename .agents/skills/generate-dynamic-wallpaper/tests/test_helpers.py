import copy
import hashlib
import json
import os
from pathlib import Path
import shutil
import struct
import subprocess
import sys
import tempfile
import unittest
import zlib

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "scripts"))
from review_wallpaper import export_overlays, inspect_png, inspect_set
from verify_wallpaper import validate_container


def write_png(path, width=9, height=7, color=(40, 80, 120)):
    def chunk(kind, data):
        return struct.pack(">I", len(data)) + kind + data + struct.pack(">I", zlib.crc32(kind + data))
    rows = (b"\0" + bytes(color) * width) * height
    path.write_bytes(b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", width, height, 8, 2, 0, 0, 0))
                     + chunk(b"IDAT", zlib.compress(rows)) + chunk(b"IEND", b""))


class HelperTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(prefix="wallpaper-helper-test-")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.entries = [
            {"fileName": "night.png", "time": "00:00:00", "isForDark": True},
            {"fileName": "master.png", "time": "12:00:00", "isPrimary": True},
            {"fileName": "morning.png", "time": "09:00:00", "isForLight": True},
        ]
        for index, entry in enumerate(self.entries):
            write_png(self.root / entry["fileName"], color=(index * 70, 80, 120))
        self.manifest = self.root / "daynight.json"
        self.manifest.write_text(json.dumps(self.entries))
        self.expected = inspect_set(self.manifest, self.root / "master.png", (9, 7))

    def container(self):
        return {
            "count": 3, "primaryIndex": 0,
            "frames": [{"index": i, "width": 9, "height": 7, "decoded": True} for i in range(3)],
            "metadata": {"ap": {"l": 2, "d": 1},
                         "ti": [{"i": 2, "t": 0.375}, {"i": 0, "t": 0.5}, {"i": 1, "t": 0.0}]},
        }

    @unittest.skipUnless(os.environ.get("DAYNIGHT_BIN_DIR"), "Set DAYNIGHT_BIN_DIR to test built executables")
    def test_packaged_verification_workflow(self):
        binaries = Path(os.environ["DAYNIGHT_BIN_DIR"]).resolve()
        suffix = ".exe" if sys.platform == "win32" else ""
        for index, entry in enumerate(self.entries):
            write_png(self.root / entry["fileName"], width=256, height=256,
                      color=(index * 70, 80, 120))
        heic = self.root / "wallpaper.heic"
        subprocess.run([str(binaries / ("daynight" + suffix)), "-i", str(self.manifest),
                        "-o", str(heic)], check=True, capture_output=True, text=True)
        evidence = self.root / "verification"
        verifier = Path(__file__).resolve().parents[1] / "scripts/verify_wallpaper.py"
        subprocess.run([sys.executable, str(verifier), str(self.manifest), str(heic),
                        "--master", str(self.root / "master.png"), "--expected-size", "256x256",
                        "--output", str(evidence), "--inspector",
                        str(binaries / ("daynight-inspect" + suffix))],
                       check=True, capture_output=True, text=True)
        report = json.loads((evidence / "verification.json").read_text())
        self.assertEqual(report["technicalValidation"], "pass")
        self.assertEqual(report["container"]["count"], 3)
        self.assertEqual(report["container"]["metadata"]["ap"], {"l": 2, "d": 1})

    def test_maps_appearance_to_primary_first_encoded_order(self):
        self.assertEqual(self.expected["encodedOrder"], ["master.png", "night.png", "morning.png"])
        self.assertEqual(self.expected["appearance"], {"light": 2, "dark": 1})
        self.assertEqual([f["encodedIndex"] for f in self.expected["frames"]], [1, 0, 2])

    def test_rejects_one_pixel_dimension_drift(self):
        write_png(self.root / "night.png", width=8)
        with self.assertRaisesRegex(ValueError, "Dimension mismatch"):
            inspect_set(self.manifest, self.root / "master.png", (9, 7))

    def test_rejects_unagreed_master_resolution(self):
        with self.assertRaisesRegex(ValueError, "requested size"):
            inspect_set(self.manifest, self.root / "master.png", (3840, 2160))

    def test_checks_schedule_by_index_regardless_of_entry_order(self):
        validate_container(self.expected, self.container())

    def test_rejects_wrong_schedule_appearance_dimensions_count_and_decode(self):
        mutations = [
            lambda c: c["metadata"]["ap"].update(l=1),
            lambda c: c["metadata"]["ap"].update(d=True),
            lambda c: c["metadata"]["ti"][0].update(t=0.25),
            lambda c: c["metadata"]["ti"][0].update(t=float("nan")),
            lambda c: c["metadata"]["ti"][0].update(i=0),
            lambda c: c["frames"][1].update(width=8),
            lambda c: c["frames"][1].update(decoded=False),
            lambda c: c.update(count=2),
            lambda c: c.update(primaryIndex=1),
        ]
        for mutate in mutations:
            actual = copy.deepcopy(self.container())
            mutate(actual)
            with self.subTest(actual=actual), self.assertRaises(ValueError):
                validate_container(self.expected, actual)

    @unittest.skipUnless(shutil.which("magick"), "ImageMagick is required for optional overlay export")
    def test_exports_full_coverage_native_tiles_and_wrap_without_changing_sources(self):
        before = {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in self.root.glob("*.png")}
        output = self.root / "inspection"
        report = export_overlays(self.expected, output, tile_size=4)
        comparisons = report["comparisons"]
        self.assertEqual(len([c for c in comparisons if c["kind"] == "master"]), 2)
        transitions = [c for c in comparisons if c["kind"] == "transition"]
        self.assertEqual([(Path(c["base"]).name, Path(c["candidate"]).name) for c in transitions],
                         [("night.png", "morning.png"), ("morning.png", "master.png"), ("master.png", "night.png")])
        self.assertEqual(len({c["overlay"] for c in comparisons}), 3)
        for comparison in comparisons:
            overlay = inspect_png(output / comparison["overlay"])
            self.assertEqual((overlay["width"], overlay["height"]), (9, 7))
            tiles = [inspect_png(output / tile) for tile in comparison["tiles"]]
            self.assertEqual(sorted((t["width"], t["height"]) for t in tiles),
                             sorted([(4, 4), (4, 4), (1, 4), (4, 3), (4, 3), (1, 3)]))
        self.assertEqual(before, {p.name: hashlib.sha256(p.read_bytes()).hexdigest() for p in self.root.glob("*.png")})
        self.assertEqual(report["visualReview"], "not performed by this script")
        with self.assertRaises(FileExistsError):
            export_overlays(self.expected, output, tile_size=4)


if __name__ == "__main__":
    unittest.main()
