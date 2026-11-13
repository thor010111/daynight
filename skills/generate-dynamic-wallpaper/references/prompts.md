# Prompts for one fixed scene

Replace bracketed fields with the user's scene and observations. The master image is the visual authority; the written scene description is not a substitute for providing it to the editing tool.

## Master

```text
Create one full-frame desktop wallpaper.
Scene: [user's prompt, preserving requested subjects and style].
Canvas: [requested width] × [requested height], [aspect ratio].
Camera and composition: [one fixed viewpoint and framing suitable for the scene].
Lighting: clear daytime light with readable detail and coherent shadows.
This image will be the immutable source for time-of-day relighting edits.
Use a still scene, with stable silhouettes and detailed, consistent materials.
No panel grid, contact sheet, border, watermark, or time-of-day labels.
[Any user-specific constraints, including space for desktop icons if requested.]
```

Do not add a sun disk, signage, people, or lamps just to demonstrate time changes. Preserve such elements when the user requested them.

## Relighting edit

Use the agreed, measured master dimensions below. Preserve its actual canvas; omit any rounded aspect-ratio claim that conflicts with those pixel dimensions.

```text
EDIT THE PROVIDED MASTER IMAGE. It is the source scene, not a style reference.
Return a single full-frame image with the same [master width] × [master height]
pixel canvas and composition.

Fixed scene:
- Preserve camera position, orientation, focal length, perspective, crop, horizon,
  object positions, proportions, silhouettes, occlusion, and every object count.
- Preserve architecture and window layout, terrain contours, foliage and branch
  shapes, cloud shapes, water surface pattern, and material/texture placement.
- Preserve art style, surface detail, and sharpness. Do not redraw, simplify,
  add, remove, move, resize, or replace any scene object.
- Specific anchors: [landmarks observed in the original master].

Change only the illumination to [time of day]:
[exposure, sky color, light direction, shadow direction/length/softness,
ambient light, and emission from existing fixtures, grounded in the scene plan].
Keep weather, cloud layout, season, and environmental geometry unchanged.
[Rules for visible sun/moon/stars, if part of the plan.]
Reflections and existing lights must respond plausibly to the new illumination
without changing the underlying objects. Keep dark areas readable.

This must look like the same camera observing the exact same still scene at a
different time. No alternate composition, new scene interpretation, collage,
caption, border, or label. Preserve the input pixel dimensions.
```

Example lighting progression (adapt to the scene and user's schedule):

| Phase | Illumination change |
| --- | --- |
| Dawn | Low, subdued warm light; cool ambient sky; long soft shadows |
| Morning | Brighter, clearer daylight; moderately long shadows |
| Noon | Bright neutral daylight; short shadows; master frame if appropriate |
| Sunset | Low warm light from the planned opposite direction; long shadows |
| Blue hour | Sun below horizon; blue ambient sky; existing lamps may illuminate |
| Night | Dim natural ambient light; existing fixtures visible; readable silhouettes |

The exact sun path depends on scene orientation and location. These descriptions are artistic defaults, not computed solar coordinates. Avoid mutually inconsistent shadow directions or a new moon in every frame.

## Targeted retry

Reuse the original master and complete relighting prompt. Append only the observed correction, for example:

```text
The previous attempt changed the cabin from three front windows to four and
shifted the foreground boulder right. Preserve the master's three windows and
the boulder's original outline and location. Keep all other geometry fixed.
Apply only the requested dusk illumination to the original master.
```

Do not make the failed variant the new edit target. Prompt constraints improve adherence but do not establish correctness; inspect the result.

For a dimension-only failure, name the measured output size and the required master size without inventing a scene mismatch. Preserve the original crop and all four edges; do not independently resize the candidate to make it pass. Recheck both dimensions and scene landmarks after a retry.
