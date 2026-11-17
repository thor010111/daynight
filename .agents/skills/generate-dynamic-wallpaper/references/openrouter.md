# Nano Banana Pro through OpenRouter

Use **Nano Banana Pro (`google/gemini-3-pro-image`)** with `POST https://openrouter.ai/api/v1/images` for the master, every image-to-image relighting edit, and every retry. Keep the model fixed; do not substitute a different model when a request fails. A skill-editing request does not require a paid generation run.

## Authentication

Authenticate with `Authorization: Bearer <OPENROUTER_API_KEY>` from the environment. Never put the key in saved requests, prompts, review notes, or command output. If it is not already exported, load the trusted repository-root `.env` in the same shell that runs the request:

```sh
if [ -z "$OPENROUTER_API_KEY" ]; then
  set -a
  source ./.env
  set +a
fi
```

Load it before changing to the scene output directory. Python's `os.environ` does not read `.env` automatically. Check only that the key is nonempty; do not print it, dump the environment, or enable shell tracing.

## Native resolution and endpoint capabilities

Default to `"resolution": "4K"`, `"aspect_ratio": "16:9"`, and `"n": 1`. These select a provider-native canvas, not exact 3840 × 2160 pixels. Measure the master, record its actual dimensions, and require every edit and retry to match them exactly. A previously observed native output is not a guarantee of the next response's dimensions. Do not resize, crop, or upscale outputs to make them pass.

Before generating, check the current per-endpoint records:

```sh
curl --fail --silent --show-error \
  https://openrouter.ai/api/v1/images/models/google/gemini-3-pro-image/endpoints
```

On 2026-11-17, Google AI Studio advertised `1K`, `2K`, and `4K`, whereas Google Vertex advertised only `1K` and `2K`. Both accepted `16:9`, one output, and up to fourteen input references. The native 4K request below pins the endpoint advertising that tier and disables provider fallback. Recheck capabilities when running; stop if native 4K is unavailable rather than silently accepting a lower tier or another model.

Do not send an explicit pixel `size` merely because another model supported it. Nano Banana's endpoint records advertise resolution tiers and aspect ratios, not arbitrary pixel dimensions. If exact user-supplied dimensions are unsupported, explain the limitation and obtain agreement to a native preset before generating. Prompt wording cannot force unsupported dimensions.

Save a UTF-8 `request.json` in the fresh scene folder:

```json
{
  "model": "google/gemini-3-pro-image",
  "prompt": "The complete master or relighting prompt from references/prompts.md",
  "resolution": "4K",
  "aspect_ratio": "16:9",
  "n": 1,
  "provider": {
    "only": ["google-ai-studio/global"],
    "allow_fallbacks": false
  }
}
```

For a relighting edit, add the immutable original master to that same request:

```json
{
  "input_references": [
    {
      "type": "image_url",
      "image_url": {
        "url": "https://your-existing-authorized-host.example/master.png"
      }
    }
  ]
}
```

The two blocks describe one body, not separate calls. Omit `input_references` for a fresh text-only master. Do not send chat `messages`, `modalities`, or `image_config` to this dedicated Image API. Do not invent seed, fidelity, mask, or strength controls.

## Original-master transport

Use a provider-accessible HTTPS URL or a base64 data URL containing the original master. Local paths, `file://` URLs, localhost, and conversation attachments are not API references.

- For an existing authorized HTTPS URL, verify the downloaded bytes against the saved master's SHA-256 before editing. Avoid transformed preview/CDN URLs, and restore expired links before continuing.
- Inline references use `data:image/png;base64,<encoded original PNG>` in `image_url.url`. Measure the full serialized UTF-8 body and check the current OpenRouter and Google endpoint limits. Do not inherit another provider's request-size limit. Keep inline bytes out of saved review notes and human-readable request copies; record the original-master hash instead.
- If inline transport exceeds the applicable limit, use hosting already authorized for these assets. If none exists, preserve the master and ask for an accessible URL or an authorized upload destination. Do not silently publish it, downsample it, convert it to a lossy reference, or omit it to make the request fit.

Every edit and retry must receive the same original master, never the preceding time of day.

## Execute one request and decode the actual format

The Image API returns `data[0].b64_json`; `media_type` identifies the output when known. Nano Banana may return JPEG even if `output_format: "png"` was requested. Preserve the provider bytes and decode them to PNG without resizing, cropping, grading, or changing composition. Renaming a JPEG to `.png` is not decoding it. The example below uses ImageMagick only for lossless format decoding, not relighting. Keep returned JPEGs for the gallery's `--jpeg-originals` option when their decoded pixels match the accepted PNGs.

Run this example from the scene output directory after preparing `request.json`. For an inline request, inject the original reference in memory before serialization rather than saving base64 in the human-readable request file. Choose fresh names for each attempt. Pass the locked master's dimensions as the optional argument for every edit (`python3 request_once.py WIDTHxHEIGHT`); the first master run prints dimensions for inspection and locking.

```python
import base64
import hashlib
import json
import os
from pathlib import Path
import struct
import subprocess
import sys
import urllib.request

payload = json.loads(Path("request.json").read_text(encoding="utf-8"))
if payload["model"] != "google/gemini-3-pro-image":
    raise ValueError("This workflow uses Nano Banana Pro for every request")
output = Path("master.png")  # Choose a fresh frame/attempt name for edits.
metadata = output.with_suffix(".api.json")
raw_output = output.with_suffix(".provider-image.bin")
if any(path.exists() for path in (output, metadata, raw_output)):
    raise FileExistsError("Choose fresh output filenames")
body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
print(f"Serialized request: {len(body)} bytes")
request = urllib.request.Request(
    "https://openrouter.ai/api/v1/images", data=body,
    headers={"Authorization": "Bearer " + os.environ["OPENROUTER_API_KEY"],
             "Content-Type": "application/json"}, method="POST",
)
with urllib.request.urlopen(request, timeout=600) as response:
    result = json.load(response)
images = result.get("data", [])
if len(images) != 1:
    raise ValueError("Expected exactly one generated image")
item = images[0]
original = base64.b64decode(item["b64_json"], validate=True)
raw_output.write_bytes(original)
if original.startswith(b"\x89PNG\r\n\x1a\n"):
    output.write_bytes(original)
else:
    subprocess.run(["magick", str(raw_output), str(output)],
                   check=True, capture_output=True, text=True)
png = output.read_bytes()
if len(png) < 24 or png[:8] != b"\x89PNG\r\n\x1a\n" or png[12:16] != b"IHDR":
    raise ValueError("Expected decoded PNG bytes; retain provider output")
actual = struct.unpack(">II", png[16:24])
metadata.write_text(json.dumps({
    "model": payload["model"], "requested_resolution": payload["resolution"],
    "requested_aspect_ratio": payload["aspect_ratio"], "actual_size": list(actual),
    "provider_media_type": item.get("media_type"),
    "provider_sha256": hashlib.sha256(original).hexdigest(),
    "sha256": hashlib.sha256(png).hexdigest(), "request_bytes": len(body),
    "created": result.get("created"), "usage": result.get("usage"),
}, indent=2) + "\n", encoding="utf-8")
print(f"Measured native output: {actual[0]} x {actual[1]}")
if len(sys.argv) > 1:
    expected = tuple(map(int, sys.argv[1].split("x")))
    if actual != expected:
        raise ValueError(f"Size mismatch: got {actual}, expected {expected}; retain failed attempt")
```

Keep network retries manual: an ambiguous timeout should not immediately trigger another paid generation. Use the main skill's two-retry limit for rejected frames, always with Nano Banana Pro and the original master. Record returned cost rather than promising a fixed price. API success and matching dimensions do not pass visual review; complete the landmark comparisons, HEIC packaging, and decoded-frame verification.

## Sources

Verified on 2026-11-17:

- [Nano Banana Pro model and editing capabilities](https://openrouter.ai/google/gemini-3-pro-image)
- [Image API requests, provider routing, references, resolution, and response format](https://openrouter.ai/docs/guides/overview/multimodal/image-generation)
- [Current Nano Banana Pro endpoint capabilities](https://openrouter.ai/api/v1/images/models/google/gemini-3-pro-image/endpoints)
