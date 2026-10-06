# BebekonCode identity

The product name is **BebekonCode**. Keep `com.bebekon.agent-workspace` stable for upgrades and
existing local data. The mascot is a snowman wearing round glasses, a blue scarf and a top hat.

Source asset: `public/brand/snowman.png`, generated with the built-in ImageGen tool on 2026-10-06
with `transparent_background: true`. No API/CLI fallback was used. The full PNG is the preserved
generation output. `tauri icon` derives Windows ICO and PNG packaging assets from it.

Regenerate packaging sizes with:

```powershell
npm run tauri -- icon public/brand/snowman.png --output src-tauri/icons
```

Final generation prompt:

> Create a polished brand mascot logo for a desktop AI coding workspace called BebekonCode. The image itself must contain NO text or letters. A friendly confident snowman wearing thick dark round eyeglasses with clear lenses (eyes visible), a charcoal black top hat with a subtle indigo hatband, and a vivid indigo-blue scarf wrapped around the neck with a short flowing end. Small orange carrot nose, gentle smile, two simple charcoal buttons. White snow with very subtle cool blue shading. Compact centered upper-body snowman silhouette, designed to be instantly recognizable as a desktop application icon at small sizes. Premium clean vector-like illustration rendered as a raster: bold clear shapes, smooth crisp edges, minimal details, balanced silhouette, no photorealism, no scenic background, no snowflakes, no ground, no border, no text, no watermark. Square composition, generous even transparent margin around the entire mascot, true transparent alpha background. The hat, glasses, scarf and snowman must all be visible and recognizable. A charming modern software brand asset, suitable on dark graphite UI and light backgrounds.
