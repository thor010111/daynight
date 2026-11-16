#!/usr/bin/env python3
"""Render the gallery as plain HTML. Visitors need no JavaScript or build tools."""

import argparse
from html import escape
import json
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
TEMPLATE = ROOT / "scripts/templates/gallery.html"
DOWNLOAD_ICON = '<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3v12m-4-4 4 4 4-4M5 16v5h14v-5"/></svg>'


def read_catalog(site):
    path = site / "catalog.json"
    scenes = json.loads(path.read_text(encoding="utf-8")) if path.exists() else []
    if not isinstance(scenes, list):
        raise ValueError("Catalog must be a list.")
    return scenes


def size(file):
    return f"{file['bytes'] / 1024 / 1024:.1f} MB"


def link(file, text, css="text-link", download=True):
    attribute = ' download' if download else ''
    return f'<a class="{css}" href="{escape(file["url"], quote=True)}"{attribute}>{text}</a>'


def render_feature(scene):
    title = escape(scene["title"])
    width, height = int(scene["width"]), int(scene["height"])
    variants = []
    for appearance, flag in (("day", "light"), ("night", "dark")):
        frame = next(frame for frame in scene["frames"] if frame[flag])
        picture = (f'<img src="{escape(frame["preview"], quote=True)}" '
                   f'alt="{title} — {escape(frame["label"])}" width="{width}" height="{height}" '
                   f'fetchpriority="high">')
        variants.append(link(frame["original"], picture,
                             f"hero-frame hero-{appearance}", download=False))
    hours = '\n'.join(f'<a href="#{escape(scene["id"], quote=True)}-frame-{index}">{escape(frame["time"][:5])}'
                      f'<span>{escape(frame["label"])}</span></a>'
                      for index, frame in enumerate(scene["frames"]))
    heic = link(scene["heic"], f'{DOWNLOAD_ICON}Download dynamic wallpaper'
                f'<span class="button-detail">HEIC · {size(scene["heic"])}</span>', "button")
    return f'''<section class="featured wrap" aria-label="Featured wallpaper">
      <div class="feature-top"><span class="eyebrow">A place to stay awhile</span><span class="muted">Day &amp; night</span></div>
      <div class="landscape" style="aspect-ratio: {width} / {height}">
        {''.join(variants)}
        <div class="image-shade"></div>
        <div class="scene-caption"><span class="eyebrow">Now visiting</span><h2>{title}</h2></div>
      </div>
      <nav class="hour-links" aria-label="Browse this scene by time of day">{hours}</nav>
      <div class="feature-bottom"><p>{escape(scene["description"])}</p>{heic}</div>
    </section>'''


def render_collection(scene):
    title, slug = escape(scene["title"]), escape(scene["id"], quote=True)
    width, height = int(scene["width"]), int(scene["height"])
    cards = []
    for index, frame in enumerate(scene["frames"]):
        label = escape(frame["label"])
        preview = (f'<img src="{escape(frame["preview"], quote=True)}" alt="{title} — {label}" '
                   f'width="{width}" height="{height}" loading="lazy" decoding="async">'
                   '<span class="preview-hint" aria-hidden="true">↗</span>')
        image = link(frame["original"], preview, "frame-preview", download=False)
        extension = frame["original"]["url"].rsplit(".", 1)[-1].upper()
        download = link(frame["original"], f'{escape(extension)} {DOWNLOAD_ICON}', "frame-download")
        cards.append(f'''<article class="frame-card" id="{slug}-frame-{index}">
          {image}
          <div class="frame-bottom"><h4 class="frame-name">{label}<span class="frame-time">{escape(frame["time"][:5])}</span></h4>{download}</div>
        </article>''')
    heic = link(scene["heic"], 'Download dynamic wallpaper <span aria-hidden="true">↓</span>')
    return f'''<section class="collection-block" aria-labelledby="{slug}-title">
        <div class="collection-top"><div><h3 id="{slug}-title">{title}</h3><p>{len(scene["frames"])} moments · {width} × {height} · AI illustrated</p></div></div>
        <div class="frame-grid">{''.join(cards)}</div>
        <div class="collection-download">{heic}<span>macOS · HEIC · {size(scene["heic"])}</span></div>
      </section>'''


def render_page(scenes):
    feature = render_feature(scenes[0]) if scenes else ''
    collections = '\n'.join(render_collection(scene) for scene in scenes)
    if not scenes:
        collections = '<p>The first little world is on its way.</p>'
    return TEMPLATE.read_text(encoding="utf-8").replace("<!-- FEATURE -->", feature).replace("<!-- COLLECTIONS -->", collections)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("site", nargs="?", type=Path, default=ROOT / "site")
    args = parser.parse_args()
    (args.site / "index.html").write_text(render_page(read_catalog(args.site)), encoding="utf-8")
    print(f"Rendered static gallery: {args.site / 'index.html'}")


if __name__ == "__main__":
    main()
