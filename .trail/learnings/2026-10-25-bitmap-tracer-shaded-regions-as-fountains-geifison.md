---
title: "Bitmap tracer: shaded regions as fountains, smooth outlines, clean ground edge"
author: Geifison
date: 2026-10-25
branch: main
cost: { minutes: 29, tokens: 32447 }
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
- Colour outline modes merge palette pieces across soft borders (mean Oklab step <= 0.03+0.03d) while a quadratic colour surface in sRGB still fits (added MSE <= (0.012+0.02d)^2); regions then get a flat colour or a linear Fountain with up to 8 stops (trace_region.rs). Settings.gradients (dialog checkbox, key dialog.trace_gradients).
- Colour model in sRGB, not Oklab: Fountain "Direct" blends in sRGB, so authored gradients are planes there but curved in Oklab (a plane fit in Oklab refused red->blue).
- Small pictures (< 1600 px long side, k = min(4, 1600/long)): each layer MASK is softened (1 6 1), bilinear upsampled k times, cut at 0.5 (0.35 inside the mask so thin lines live); FitParams.scale = k widens the corner reach.
- Background flood: step tested also over 3 px on the same line, far test on 5x5 mean, then 3x3 majority passes. Fixed ragged ground edge under dark soft shapes.
- trace_a_file takes TRACE_SCALE and TRACE_CROP for inspection; TRACE_IN must be an absolute path (cwd is the crate).

## Tried and ruled out
- Enlarging + Gaussian-blurring the COLOUR image before tracing: blur makes soft edges that the gradient merge reads as ramps (rings round shapes, 4 shapes for a square). Smooth the per-layer mask instead.
- Plane (linear) merge criterion: leaves seams inside gently curved shadings; quadratic fits from additive moments.

## Watch out
- Fountain linear axis always passes through the bounds' centre: compare ramp ends only along the axis direction.
- Fill bounds are the page-space path (group transform included): ops2 maps ramp ends with transform * rect_affine.
- help_shots::print_dialog_tabs_fit_without_scrolling can fail in the full parallel run and pass alone.
- Thin seams can remain between neighbouring ramps whose union is not quadratic (rainbow bands).
