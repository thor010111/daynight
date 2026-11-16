# Riverflow through OpenRouter

Use `sourceful/riverflow-v2.6-pro` with `POST https://openrouter.ai/api/v1/images` for both text-to-image and image-to-image requests. Authenticate with `Authorization: Bearer <OPENROUTER_API_KEY>` from the environment. Never put the key in saved request JSON, prompts, review notes, or command output. A skill-editing request does not require a paid generation run.

## Load the local credentials

The daynight repository stores `OPENROUTER_API_KEY` in its root `.env`, which is ignored by Git. If the variable is not already exported, load that trusted local file from the repository root in the same shell that will run the API request:

```sh
set -a
source ./.env
set +a
```

Load it before changing to the scene output directory. Python's `os.environ` does not read `.env` automatically. Check only that the key is nonempty; do not print it, dump the environment, or enable shell tracing while loading it. Preserve an already exported key instead of replacing it with `.env`.

## Native dimensions

Request explicit pixels, defaulting to `"size": "3840x2160"`, with `"output_format": "png"` and `"n": 1`. Use the same size for the master and all edits. Sourceful V2.6 accepts custom sizes up to 16,384 pixels per edge, 16.8 megapixels total, and a 16:1 aspect ratio. Check current endpoint capabilities if a requested canvas is rejected.

**Omit `resolution` and `aspect_ratio` when using an explicit pixel `size`.** Sourceful V2.6 rejects combining them. The alternative `resolution: "4K"` plus `aspect_ratio: "16:9"` selects a provider-defined canvas; it does not establish the required 3840 × 2160 output. Prompt wording is supplementary. Measure the returned PNG and reject a size mismatch without resizing it.

Save a UTF-8 `request.json` in the scene's output folder with the actual master or relighting prompt:

```json
{
  "model": "sourceful/riverflow-v2.6-pro",
  "prompt": "The complete scene or relighting prompt from references/prompts.md",
  "size": "3840x2160",
  "output_format": "png",
  "n": 1
}
```

For a user reference or a relighting edit, add this field to that same request. Every relighting request references the immutable original master:

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

The two JSON blocks describe one request body; `input_references` is not a separate API call. Omit it for a fresh master without a user reference. Do not send chat `messages`, `modalities`, or `image_config` to this dedicated Image API. Leave optional reasoning/provider tuning unset unless the task requires it and current endpoint documentation supports it.

## Original-master transport

Sourceful documents a **4.5 MB request-size limit** and recommends image URLs over base64. A detailed native 4K PNG can exceed that limit after base64 encoding.

- Prefer an existing provider-accessible HTTPS URL containing the original master. Verify that its downloaded bytes match the saved master's SHA-256 before editing; avoid transformed preview/CDN URLs. Local paths, `file://` URLs, localhost, and conversation attachments are not provider-accessible references.
- Inline references use `data:image/png;base64,<encoded original PNG>` in `image_url.url`. Measure the full serialized UTF-8 request body, including base64 and prompt, and keep it below the documented limit. Do not print or retain inline image data in review notes.
- If inline transport is too large, use hosting already authorized for these assets. If none exists, preserve the master and ask for an accessible URL or an authorized upload destination. Do not silently publish it, downsample it, convert it to a lossy reference, or omit it to make the request fit.

## Execute one request and save the result

The following standard-library Python example reads `request.json` in the current scene output directory, makes one request, and saves the returned PNG and usage metadata. Choose fresh filenames for each attempt. Keep network retries manual: an ambiguous timeout should not immediately trigger another paid generation. Apply the skill's two-retry limit to rejected frames.

```python
import base64
import json
import os
from pathlib import Path
import struct
import urllib.request

payload = json.loads(Path("request.json").read_text(encoding="utf-8"))
output = Path("master.png")  # Use a fresh frame/attempt filename for edits.
metadata = output.with_suffix(".api.json")
if output.exists() or metadata.exists():
    raise FileExistsError("Choose fresh output filenames before requesting an image")
body = json.dumps(payload, ensure_ascii=False).encode("utf-8")
if len(body) >= 4_500_000:
    raise ValueError("Use an authorized HTTPS reference; request exceeds 4.5 MB")
request = urllib.request.Request(
    "https://openrouter.ai/api/v1/images",
    data=body,
    headers={
        "Authorization": "Bearer " + os.environ["OPENROUTER_API_KEY"],
        "Content-Type": "application/json",
    },
    method="POST",
)
with urllib.request.urlopen(request, timeout=600) as response:
    result = json.load(response)
images = result.get("data", [])
if len(images) != 1:
    raise ValueError("Expected exactly one generated image; inspect API status")
image = images[0]
png = base64.b64decode(image["b64_json"], validate=True)
if len(png) < 24 or png[:8] != b"\x89PNG\r\n\x1a\n" or png[12:16] != b"IHDR":
    raise ValueError("Expected PNG bytes; do not rename another format to .png")
actual = struct.unpack(">II", png[16:24])
expected = tuple(int(value) for value in payload["size"].split("x"))
with output.open("xb") as stream:
    stream.write(png)
with metadata.open("x", encoding="utf-8") as stream:
    json.dump({
        "model": payload["model"],
        "requested_size": payload["size"],
        "actual_size": list(actual),
        "created": result.get("created"),
        "usage": result.get("usage"),
    }, stream, indent=2)
if actual != expected:
    raise ValueError(f"Size mismatch: got {actual}, expected {expected}; retain as failed attempt")
```

Keep the prompt/request for each attempt, the master hash, measured dimensions, and returned usage/cost alongside the review. Pricing can vary with processing, size, and edit complexity; record reported cost rather than promising a fixed per-image total. An API response and matching dimensions do not pass visual review. Continue with the landmark comparisons, HEIC packaging, and decoded-frame verification in the main skill.

## Sources

Verified against OpenRouter documentation on 2026-11-16:

- [Riverflow V2.6 Pro model and limits](https://openrouter.ai/sourceful/riverflow-v2.6-pro)
- [Image API: requests, exact sizes, references, and response format](https://openrouter.ai/docs/guides/overview/multimodal/image-generation)
- [Current Riverflow endpoint capabilities](https://openrouter.ai/api/v1/images/models/sourceful/riverflow-v2.6-pro/endpoints)
