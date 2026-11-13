import json
from pathlib import Path
import shutil
import subprocess
import sys
import tempfile
import unittest
from unittest.mock import patch
import zipfile

ROOT = Path(__file__).resolve().parents[2]
sys.path.insert(0, str(ROOT / "scripts"))
sys.path.insert(0, str(ROOT / "skills/generate-dynamic-wallpaper/tests"))
from add_wallpaper import add_wallpaper, read_catalog
from check_gallery import check
from check_gallery import PageLinks
from render_gallery import render_page
from test_helpers import write_png


class StaticPageTests(unittest.TestCase):
    def test_markup_in_titles_is_text_and_all_downloads_are_in_html(self):
        scenes = read_catalog(ROOT / "site")
        scenes[0]["title"] = '<script>alert("test")</script> & lake'
        page = render_page(scenes)
        self.assertIn('&lt;script&gt;', page)
        links = PageLinks()
        links.feed(page)
        for scene in scenes:
            for file in [scene["heic"], scene["sources"], *[frame["original"] for frame in scene["frames"]]]:
                self.assertIn(file["url"], links.downloads)
        self.assertFalse(links.anchors - links.targets)

    def test_checker_rejects_script_dependencies(self):
        for markup in ('<script src="app.js"></script>', '<a onclick="preview()">Preview</a>'):
            with self.subTest(markup=markup), self.assertRaisesRegex(ValueError, "no script"):
                PageLinks().feed(markup)


@unittest.skipUnless(shutil.which("magick"), "ImageMagick required for preview export")
class GalleryTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory()
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.site = self.root / "site"
        self.manifest = self.root / "daynight.json"
        self.master = self.root / "master.png"
        self.night = self.root / "night.png"
        write_png(self.master)
        write_png(self.night, color=(15, 20, 30))
        self.manifest.write_text(json.dumps([
            {"fileName": "master.png", "time": "12:00:00", "isPrimary": True, "isForLight": True},
            {"fileName": "night.png", "time": "00:00:00", "isForDark": True},
        ]))
        # The exporter copies an already-verified package; it does not decode HEIC.
        self.heic = self.root / "wallpaper.heic"
        self.heic.write_bytes(b"test package copied without encoding")
        (self.root / "failed.png").write_bytes(b"never publish this")

    def export(self, slug="test-scene"):
        return add_wallpaper(self.manifest, self.heic, self.master, self.site, slug,
                             "Test scene", "A description.")

    def test_delivery_preserves_sources_and_adds_working_preview_and_archive(self):
        scene = self.export()
        self.assertEqual([frame["time"] for frame in scene["frames"]], ["00:00:00", "12:00:00"])
        self.assertEqual((self.site / scene["heic"]["url"]).read_bytes(), self.heic.read_bytes())
        for frame in scene["frames"]:
            original = self.site / frame["original"]["url"]
            self.assertEqual(original.read_bytes(), (self.root / original.name).read_bytes())
            self.assertTrue((self.site / frame["preview"]).is_file())
        with zipfile.ZipFile(self.site / scene["sources"]["url"]) as archive:
            self.assertEqual(set(archive.namelist()), {"master.png", "night.png", "daynight.json"})
            self.assertEqual(archive.read("master.png"), self.master.read_bytes())
        self.assertFalse(list(self.site.rglob("failed.png")))
        check(self.site)
        original = self.site / scene["frames"][0]["original"]["url"]
        original.write_bytes(original.read_bytes()[:-1] + b"x")
        with self.assertRaisesRegex(ValueError, "Original download changed"):
            check(self.site)

    def test_new_set_appends_and_existing_downloads_are_never_replaced(self):
        self.export()
        first_catalog = (self.site / "catalog.json").read_bytes()
        with self.assertRaises(FileExistsError):
            self.export()
        self.assertEqual((self.site / "catalog.json").read_bytes(), first_catalog)
        self.export("second-scene")
        self.assertEqual([scene["id"] for scene in read_catalog(self.site)], ["test-scene", "second-scene"])
        check(self.site)

    def test_failed_preview_leaves_existing_catalog_and_downloads_untouched(self):
        self.export()
        first_catalog = (self.site / "catalog.json").read_bytes()
        with patch("add_wallpaper.subprocess.run", side_effect=subprocess.CalledProcessError(1, "magick")):
            with self.assertRaises(subprocess.CalledProcessError):
                self.export("failed-scene")
        self.assertEqual((self.site / "catalog.json").read_bytes(), first_catalog)
        self.assertFalse((self.site / "wallpapers/failed-scene").exists())
        self.assertFalse(list((self.site / "wallpapers").glob(".wallpaper-*")))
        check(self.site)

    def test_invalid_slug_and_different_dimensions_do_not_publish(self):
        with self.assertRaises(ValueError):
            self.export("../escape")
        write_png(self.night, width=8)
        with self.assertRaisesRegex(ValueError, "Dimension mismatch"):
            self.export()
        self.assertFalse((self.site / "catalog.json").exists())


if __name__ == "__main__":
    unittest.main()
