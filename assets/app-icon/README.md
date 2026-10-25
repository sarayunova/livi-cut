# LiVi Cut app icon

**Symbol:** a teal play triangle above a coral cut band: playback and the edit in one mark.

## Palette

| Colour | Hex | Used for |
|---|---|---|
| Teal | `#22cfce` | the play triangle |
| Coral | `#fd7557` | the cut band |
| Navy | `#0f1a2e` | the full-bleed tile behind the symbol |

## Geometry and style

- A 512-unit viewBox tile, rounded square `rx=112`, navy full bleed (no frame or border); the symbol
  is 62 % of the tile tall, centred on its bounding box and nudged 8 units right so the play shape
  reads as centred.
- Flat colour, no gradients or shadows, so it stays legible at 16 px.
- macOS renders (`filmcraft.icns`, `filmcraft-macos-512.png`) put the tile in Apple's grid (824 of
  1024 px, transparent margin). Windows, Linux and the web use the full-bleed tile.

## Provenance

Redrawn as clean vector geometry from a concept image made for LiVi with ChatGPT image generation (the concept and the generator
script are kept outside this repo). Licence: [LICENSE.txt](LICENSE.txt) (MIT OR Apache-2.0); the
licence covers the artwork, not the LiVi names.

## Files

| File | What |
|---|---|
| `filmcraft.svg` | canonical artwork (the tile) |
| `filmcraft-small.svg` | same artwork (kept so packaging paths stay stable) |
| `filmcraft-1024.png` | 1024 px render |
| `filmcraft-macos-512.png` | runtime Dock icon on macOS (embedded by `apps/filmcraft/src/main.rs`) |
| `filmcraft.icns` | macOS bundle icon (`CFBundleIconFile`) |
| `filmcraft.ico` | Windows icon, 16–256 px (embedded in `livi-cut.exe` by `apps/filmcraft/build.rs`) |
| `hicolor/<size>/apps/com.liviverse.livicut.png`, `hicolor/scalable/…svg` | Linux icon theme, used by `packaging/linux/com.liviverse.livicut.desktop` |

The web app's `apps/filmcraft-web/web/favicon.png` (128 px) comes from the same artwork.

## How the icon reaches each OS

- **macOS:** the window icon set at startup (`ViewportBuilder::with_icon`) is the Dock and app-switcher
  icon when the app runs unbundled. A future `.app` bundle points `CFBundleIconFile` at `filmcraft.icns`.
- **Windows:** `build.rs` embeds `filmcraft.ico` as the exe's resource icon (Explorer, Start menu,
  pinned taskbar), and the runtime icon covers the window, taskbar and Alt-Tab.
- **Linux:** the runtime icon plus `with_app_id("com.liviverse.livicut")`, which Wayland matches to
  the `.desktop` file. Install the `hicolor/` tree under `/usr/share/icons/hicolor/` and the desktop
  file under `/usr/share/applications/`.

## Regenerate

`packaging/icons.sh` (needs `resvg`; `iconutil` on macOS for the `.icns`; the `.ico` is packed by
`cargo xtask ico`). To change the artwork, replace `filmcraft.svg` and `filmcraft-small.svg`, then run
the script; on Windows or Linux build the `.icns` from the Apple-grid render with Pillow. Every derived file needs its `.attribution` sidecar
and a row in `ATTRIBUTION.md`.
