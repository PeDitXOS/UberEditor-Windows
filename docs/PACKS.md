# Effect and generator packs

An effect is not code: it's a **`manifest.json`** that declares its parameters
and an **ffmpeg template**. The same manifest drives the preview, the paused
frame and the export — there is no second implementation to keep in sync, which
is why what you see is what you get.

Built-ins are compiled into the binary. **Your own packs** live in the app's
config folder and need no build:

```
«app config»/effects/<my-effect>/manifest.json
«app config»/generators/<my-generator>/manifest.json
```

Press **`↻ packs`** (Inspector → Effects) to reload them without restarting. An
invalid manifest breaks nothing — it's reported and skipped — and a pack whose
`id` matches a core one **replaces it** (the user wins).

---

## Effect manifest

| Field | What |
|---|---|
| `id` | Unique. `core.*` is reserved for built-ins; reuse one to override it |
| `name` | Shown in the `+ Add effect…` list |
| `category` | Free-form grouping (`color`, `keying`, `blur`, `style`, `layout`…) |
| `params` | The UI: one row per entry. `"type": "float"` (with `min`/`max`/`default`) becomes a keyframable slider; `"type": "color"` becomes a color picker |
| `ffmpeg` | The filter chain. Every `{key}` from `params` is substituted |
| `notes` | Optional; shown in the Inspector |

**Substitution rules**: floats are clamped to `[min, max]` and printed without
scientific notation; colors become ffmpeg's `0xRRGGBB`; a missing parameter
falls back to its `default`. **`{u}`** is replaced by a unique counter — use it
in every internal label if your template has a `split`/`overlay` mini-graph, so
two instances of the effect in the same ffmpeg process don't collide.

Float params are **keyframable for free**: the value is evaluated at the
playhead before substitution.

```json
{
  "id": "core.gaussian_blur",
  "name": "Gaussian blur",
  "category": "blur",
  "params": [
    { "key": "sigma", "label": "Strength", "type": "float", "default": 8, "min": 0.1, "max": 60 }
  ],
  "ffmpeg": "gblur=sigma={sigma}"
}
```

## Generator manifest

Same shape, except a generator **produces** an image instead of filtering one,
so the template lives in **`source`** and two extra placeholders are reserved:
**`{d}`** (duration in seconds) and **`{fps}`**.

```json
{
  "id": "core.gradient",
  "name": "Gradient",
  "params": [
    { "key": "color_a", "label": "Color A", "type": "color", "default": "#ffb224" },
    { "key": "color_b", "label": "Color B", "type": "color", "default": "#16130f" },
    { "key": "width",   "label": "Width",   "type": "float", "default": 1920, "min": 16, "max": 4096 },
    { "key": "height",  "label": "Height",  "type": "float", "default": 1080, "min": 16, "max": 4096 }
  ],
  "source": "gradients=s={width}x{height}:c0={color_a}:c1={color_b}:x0=0:y0=0:x1={width}:y1={height}:speed=0:rate={fps}:duration={d}"
}
```

A generator clip is a **normal clip**: position, scale, rotation, opacity and
keyframes all apply on top.

---

## Built-ins

**Effects**

| id | Name | Parameters |
|---|---|---|
| `core.color_correct` | Color correction | Brightness, Contrast, Saturation, Gamma |
| `core.chroma_key` | Chroma Key | Key color, Similarity, Blend, Despill |
| `core.gaussian_blur` | Gaussian blur | Strength (sigma) |
| `core.drop_shadow` | Drop shadow | Colour, Offset X/Y, Softness, Opacity, Room |
| `core.vertical_fill` | Vertical: blurred background | Width, Height, Blur — the Shorts/Reels look, used by **📱 Vertical** |

**Generators**

| id | Name | Parameters |
|---|---|---|
| `core.solid` | Solid rectangle | Color, Width, Height |
| `core.gradient` | Gradient | Color A, Color B, Width, Height |

### Why chroma key looks the way it does

`core.chroma_key` is not a bare `chromakey=` call: ffmpeg's `chromakey`
**overwrites** the alpha plane, which turned the generated avatar's transparent
canvas into a black veil in the paused preview and the export. The template
multiplies the source's own alpha by the key's alpha instead — final alpha =
source × key — which is exactly what the webview compositor does per pixel.

TTS engines use the same "drop a JSON manifest, no code" idea — see
[`TTS.md`](TTS.md).
