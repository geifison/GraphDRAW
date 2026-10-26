---
title: Precise two-tone tracing measured against a reference SVG
author: Geifison
date: 2026-10-25
branch: main
cost: { minutes: 168, tokens: 579563 }
touches:
  - AGENTS.md
  - apps/graphdraw/lang/ar.toml
  - apps/graphdraw/lang/bn.toml
  - apps/graphdraw/lang/cs.toml
  - apps/graphdraw/lang/de.toml
  - apps/graphdraw/lang/en.toml
  - apps/graphdraw/lang/es.toml
  - apps/graphdraw/lang/fr.toml
  - apps/graphdraw/lang/hi.toml
  - apps/graphdraw/lang/id.toml
---

## Decided
- Two-tone pictures (auto-detected in colour presets) and black and white mode go through trace_ink.rs: per-pixel ink amount between Otsu class means, Catmull-Rom upsampled k (<=8) times, cut at 0.5; tolerance 0.2+0.6*(1-detail/100); one shape per outer ring with its holes.
- Two-tone test: 2-means in Oklab, 98.5% of pixels within 0.06 of the mix, and <=12 off-mix pixels with 7-8 off-mix neighbours (JPEG noise is scattered along edges; a third colour is a patch).
- Hard-edged (no anti-aliasing) input is softened 0.2/0.6/0.2 before the isoline (the user's "blur 1-2 px" rule, applied to the ink field, not the colours).
- snap_corner uses least-squares lines over points K/2..2K from the corner; two-point lines on a fine grid tilted corners by up to 0.4 px.
- Challenge vetorizar/images.jpg vs images.svg: Quick Trace via app export gives IoU 0.9675, 13 paths, 139 nodes (ref 149); against the jpg itself ours is IoU 0.984 / edge 0.12 px, the reference 0.972 / 0.20 px.
- Bench: scratchpad node + @resvg/resvg-js + pngjs compare.mjs (IoU, bias, edge distances, nodes); tests trace::tests::trace_to_svg and quick_trace_to_svg (use --exact: the name of one contains the other).

## Tried and ruled out
- Iterative back-projection (correcting each pixel's covered share): exact on synthetic edges but amplifies JPEG noise (360-630 nodes, IoU 0.956).
- Tracing B/W before the bilateral step: noisy mid tones in colour pictures gave 584 shapes.
- TWO_TONE_SHARE 0.997: the caduceus JPEG fails it; use the patch test instead.

## Watch out
- The reference SVG is itself ~0.2 px off the jpg (shifted up, thinner); do not tune toward it beyond the jpg.
- B/W of a logo on a dark ground draws the light parts in white (ink = tone the border is not): invisible on a white page.
- Edge positions from interpolated coverage can be off by up to 1/8 px.
