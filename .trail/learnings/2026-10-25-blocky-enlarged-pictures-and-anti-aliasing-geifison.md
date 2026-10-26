---
title: Blocky (enlarged) pictures and anti-aliasing coverage in the tracer
author: Geifison
date: 2026-10-25
branch: main
cost: { minutes: 27, tokens: 30297 }
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
- trace() first looks for a block grid (trace_grid.rs): column/row colour-change counts, largest block 3..64 px whose grid carries 90% of changes within clamp(0.15b,0.6,1.25) px, then the size within 15% below with least deviation; interior pixels must match block centres. Traced from one pixel per block, shapes mapped back by Affine(bx,by,x0,y0).
- Blocky mode (unit > 1): no bilateral, specks measured in user-visible pixels, fringe rule unbounded in thickness, removed ground is a blend partner, regions within 0.12 of ground become ground, no gradients.
- Layer contours for small pictures use per-pixel COVERAGE (blend t of the pixel colour between the layer and its neighbour), softened 1-6-1, bilinear k-times, cut at 0.5; coverage extended past the picture edge and contour clamped to it.
- User's reference: pixelated "des" (blocks ~9.3 px) now traces to one orange shape, 21 nodes; synthetic AA quarter ring enlarged 10x traces to 5 nodes, area within 5%.

## Tried and ruled out
- Fixed +-0.75 px grid slack: misses pictures enlarged then resampled (each grid line smeared over 2 px).
- Taking the largest passing block size: on few blocks a slightly wrong size (10.25 vs 10) passes; pick least deviation near it.
- Phase in [0,b): a phase of b-0.001 drops the first column; fold phases near b to negative.

## Watch out
- fringe param multiplies perimeter: use saturating_mul (usize::MAX/8 marks blocky mode).
- graphdraw-text forced_justification_spreads_the_last_line_too failed on 2026-10-25 without changes to crates/text (pre-existing).
- In blocky mode with ground removal, dark details near the ground colour inside shapes become holes.
