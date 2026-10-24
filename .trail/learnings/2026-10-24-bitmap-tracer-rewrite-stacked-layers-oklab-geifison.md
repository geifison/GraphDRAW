---
title: "Bitmap tracer rewrite: stacked layers, Oklab, corner-aware fitting"
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 28, tokens: 24223 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/src/app.rs
  - apps/graphdraw/src/contrast.rs
  - apps/graphdraw/src/main.rs
  - apps/graphdraw/src/ops2.rs
  - apps/graphdraw/src/trace.rs
  - apps/graphdraw/src/ui/align_docker.rs
  - apps/graphdraw/src/ui/dialogs.rs
  - apps/graphdraw/src/ui/svg_icons.rs
---

## Decided
- Tracer split: trace.rs (outline pipeline, presets), trace_color.rs (Oklab palette, bilateral, background flood, speck/fringe clean), trace_fit.rs (corners + least-squares cubics with Newton reparam), trace_center.rs (skeleton graph, spur prune, junction pairing, per-stroke width from chamfer distance).
- Traced carries its own stroke width (width > 0 = stroke); the old thread_local last_stroke_width_px is gone.
- Background removal floods from the border through small Oklab steps (follows gradients), plus enclosed pixels within 0.05 of the ground (letter counters). Palette is built after the ground is cleared.
- Stacked output (largest area at bottom, each layer grown 2 px under layers above + holes under them filled) avoids seams between colours.
- Centreline ink = the tone the border is not (light lines on dark work).
- Visual check: TRACE_IN/TRACE_OUT + `cargo test --release -p graphdraw trace_a_file -- --ignored --nocapture`.

## Tried and ruled out
- Fixed 128 threshold and removing a single border colour: gradient grounds left big rings and wasted the palette.
- Gaussian blur prefilter: rounds corners; replaced with bilateral.

## Watch out
- Enclosed-ground removal also clears small dark details close to the ground colour (pen nib dot on the brand logo).
- 4 MP logo takes ~4-5 s in release; gradients still band (no gradient fills yet).
- Clock skew on this box made cargo reuse stale test binaries: touch sources if test names look old.
