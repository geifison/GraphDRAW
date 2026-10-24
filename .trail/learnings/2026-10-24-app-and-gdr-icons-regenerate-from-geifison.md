---
title: App and .gdr icons regenerate from SVG sources
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 10, tokens: 100210 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/src/contrast.rs
  - assets/brand/app-1024.png
  - assets/brand/app-128.png
  - assets/brand/app-16.png
  - assets/brand/app-20.png
  - assets/brand/app-24.png
  - assets/brand/app-256.png
  - assets/brand/app-32.png
---

## Decided
- Brand sources are assets/brand/app.svg (the logo) and gdr.svg (page with folded corner, logo nested via inner <svg>).
- scripts/brand/render-icons.mjs renders {app,gdr}-{16..1024}.png and PNG-entry .ico (16-256); then scripts/brand/make-assets.ps1 makes the MSIX tiles. Run render-icons from a dir where `npm i --no-save @resvg/resvg-js` was done (resolved from cwd).
- Icon consumers: build.rs (app.ico into exe), ui/brand.rs (app-256.png window/about/welcome), graphdraw.iss, build-dmg.sh, build-appimage.sh, scripts/web/build.sh, msix Assets.
## Tried and ruled out
- No rsvg/inkscape/magick/python on the Windows box; node + @resvg/resvg-js works.
## Watch out
- splash.jpg has the app icon baked into the raster (tile at about x 253-475, y 107-312 of 1376x768); a logo change needs a one-off composite (blurred background patch + new logo) and `make-assets.ps1 -Splash <png>`.
- Windows icon cache can keep showing the old exe icon.
