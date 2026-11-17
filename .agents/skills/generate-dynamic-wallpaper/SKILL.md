---
name: generate-dynamic-wallpaper
description: Generate native 4K dynamic desktop wallpapers with Nano Banana Pro (google/gemini-3-pro-image) through OpenRouter by relighting one fixed master scene across times of day. Use for matching day/night wallpaper sets and macOS dynamic HEIC wallpapers, with visual drift review and daynight packaging.
---

# Generate a dynamic wallpaper

Create one scene, then edit its lighting for each time of day. The scene's camera, geometry, objects, materials, textures, and composition must stay fixed. The user prefers AI relighting with visual checks over scripted color grading.

Treat matching as an acceptance requirement, not a capability guaranteed by prompting. AI edits can change details. Never claim pixel-exact geometry or automatic proof of consistency; report what was actually inspected. If visible drift remains, the set is unfinished.

## Establish the scene

Use the user's prompt, reference, dimensions, schedule, and destination. Infer macOS HEIC when working in the daynight repository. Otherwise ask about the platform only if it affects the deliverable. Default to a landscape 16:9 scene and six frames: night 00:00, dawn 06:00, morning 09:00, noon 12:00, sunset 18:00, and blue hour 20:00. These are editable clock times, not calculated sunrise/sunset times.

State the chosen schedule and resolution before generation. Default to Nano Banana Pro's native `resolution: "4K"` and `aspect_ratio: "16:9"` preset, then measure and report the returned pixel dimensions; a preset does not promise exact 3840 × 2160. Use those same API settings for every edit. If the user supplies exact pixels, first check whether the endpoint supports them. If it does not, explain that limitation and obtain agreement to the native preset before generation. Do not invent exact-size support or switch models to satisfy a size request. For the default landscape preset, reject output below 3840 × 2160 rather than treating the requested 4K label as proof. Smaller output fails the native 4K requirement; do not upscale, resize, or crop it to pass.

Once the native master is measured and accepted, lock its exact pixel dimensions for all edits and checks. Do not also demand an exact aspect ratio that conflicts with them: an accepted 1672 × 941 canvas, for example, is only approximately 16:9. A one-pixel mismatch still requires a retry or an explicitly agreed change; stronger prompt wording does not guarantee size control.

Create a fresh output folder, usually `output/wallpapers/<scene>/`, without replacing an existing set. Keep the master, accepted frames, prompts, review notes, comparison page, manifest, and final HEIC there. Retain failed attempts separately from accepted frames.

## Generate and lock the master

Use **Nano Banana Pro (`google/gemini-3-pro-image`) through OpenRouter** for the master, every relighting edit, and every retry. Read [references/openrouter.md](references/openrouter.md) before generation for authentication, supported resolution settings, original-master references, response decoding, and transport limits. Load `OPENROUTER_API_KEY` from the repository-root `.env` when it is not already exported; never display its value. This skill uses that API directly; built-in image generation and upscaling are not part of this workflow. Keep this model fixed throughout the run. Do not use another model as a fallback unless the user explicitly requests it. If `OPENROUTER_API_KEY` or API access is unavailable, report the blocker and preserve completed work.

Use [references/prompts.md](references/prompts.md) to make the master and relighting prompts. Generate a single full-frame image, never a collage of times of day. Favor a clear daytime master that exposes detail for inspection. Do not remove user-requested scene elements merely to simplify matching.

Inspect the master before deriving frames. Save the provider's original bytes and decode them into the output folder as `master.png`; Nano Banana may return JPEG even when PNG was requested. Convert the format without resizing, cropping, grading, or changing composition; do not merely rename a non-PNG file. Use the master itself for the matching daytime slot when appropriate. Record its SHA-256 and measured dimensions, model, requested API resolution/aspect ratio, available usage/cost, the original user prompt, the generation prompt, and the scene constraints in `review.md`. Keep credentials and inline image data out of review notes.

Make a short scene-specific list of landmarks visible in the master: horizon height, building corners, window count, mountain ridges, tree silhouettes, paths, foreground objects, and distinctive texture details. Use locations or approximate image coordinates to make later comparison concrete. Also record the allowed lighting changes and a coherent sun/shadow trajectory. Freeze weather, cloud shapes, vegetation, water surface geometry, season, and material patterns; changing the clock alone does not authorize changing these.

Do not require approval of the master unless the user asks for a preview/approval workflow. Once selected, keep it immutable. If the master must change, invalidate and regenerate its derived frames.

## Relight from the same source

Generate each time of day as an **edit of the original master**. Never generate variants independently from text and never feed the preceding time of day into the next one: repeated edits accumulate drift.

Inspect local input images before editing. Supply the original master in OpenRouter's `input_references` using a provider-accessible HTTPS URL or a base64 data URL within the request-size limit, as described in [references/openrouter.md](references/openrouter.md). A local path or conversation attachment is not an API reference. Send the same original master and native 4K resolution/aspect-ratio settings on every edit and retry, and require the output to match the measured master dimensions. If that reference is no longer accessible, restore it before continuing. Never invent unsupported seed, mask, strength, or fidelity parameters, and never treat a repeated seed as a geometry guarantee.

Reuse the same invariant prompt verbatim, changing only the lighting specification. Allow exposure, color temperature, sky illumination, physically coherent shadows, highlights, and illumination of existing fixtures. Sun, moon, and stars may change visibility/position only according to the scene plan; they must not cause scene rearrangement. Keep the original art style and level of detail.

Save each output as a separate full-resolution PNG. Check each frame against the master before continuing, so a failed method does not produce an entire unusable set. If a frame drifts, retry from the original master with a precise description of the changed feature. Default to at most two retries per failed frame. If it still fails, preserve accepted work, explain the remaining mismatch, and stop before producing a final wallpaper. Do not lower the matching requirement silently.

## Review the entire set

The bundled `scripts/review_wallpaper.py` checks the time manifest and PNG header dimensions, and creates a local comparison page. Its optional `--export-overlays` mode uses ImageMagick to create full-frame overlays, native-size tiles and neighboring-time comparisons in a fresh directory. These are inspection aids; accepted source images are never altered and no visual pass is assigned. Read [references/daynight.md](references/daynight.md) for the manifest and commands.

Inspect actual rendered images, not just filenames, tool success, or a thumbnail contact sheet:

1. Compare every frame with the master using overlays or blink at full frame and at 100% zoom, including corners and detailed landmarks. Check for doubled contours, changed counts, shifted textures, new objects, missing details, crop, scale, perspective, or horizon movement. A global similarity score cannot certify this under changing illumination.
2. Separate expected light/shadow boundaries from object boundaries. Reflections may brighten or change color; reflected objects must still have the same structure and placement. Darkness is not evidence that hidden geometry matches. Mark regions that cannot be assessed as uncertain rather than passed.
3. Compare neighboring times and the last-to-first transition. Look for scene jumps, style changes, inconsistent light direction, flickering cloud shapes, or unrelated weather changes. Keep nighttime readable without making it look like daylight tinted blue.
4. Record each frame's inspected landmarks, hash, result (`pass`, `fail`, or `uncertain`), and any unresolved mismatch in `review.md`. Re-review replacements. Do not mark frames passed without opening them. Only package a set whose required checks pass; otherwise explain what remains unverified.

Use the comparison page in a browser if available. If browser interaction is unavailable, inspect the files directly with available image tools and state the reduced review coverage. Do not invent a completed blink/overlay review. An unperformed required check remains unfinished.

Render inspection images in small batches so tool output limits do not omit them. Save each frame's review result before proceeding; an exported or omitted image is not an inspected image.

## Package and deliver

For macOS, follow [references/daynight.md](references/daynight.md): validate the accepted frame set, produce the time-based manifest, build the HEIC with daynight, then run `scripts/verify_wallpaper.py` to decode every frame and check dimensions, schedule and appearance indices. Open its primary/Light/Dark previews to confirm their visual assignments. The reference also covers recovery from restricted macOS image-service access. Do not change the desktop wallpaper unless asked.

For other platforms, deliver the accepted PNGs and requested schedule format; do not claim an unsupported native dynamic-wallpaper package exists. If the HEIC encoder is unavailable, deliver the PNGs, manifest, review, and exact packaging command, clearly marking HEIC generation incomplete.

When working in the daynight repository, also copy each finished, reviewed and verified set into its GitHub Pages gallery with `scripts/add_wallpaper.py` at the repository root. Follow the [gallery delivery instructions](references/daynight.md#github-pages-gallery). Keep original downloads unchanged and copy only accepted frames. Updating the local gallery is part of delivery; publishing commits to GitHub requires authorization from the current task.

Return links to the final files and comparison page, actual resolution and frame count, and a short account of visual review and packaging validation. Distinguish successful HEIC encoding from observing real macOS wallpaper switching. Include known limitations and the saved prompt set. Use wording such as “no drift found in the inspected landmarks,” never “guaranteed identical.”
