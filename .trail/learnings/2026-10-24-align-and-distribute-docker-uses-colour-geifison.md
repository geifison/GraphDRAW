---
title: Align and Distribute docker uses colour SVG pictures
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 9, tokens: 74743 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/src/app.rs
  - apps/graphdraw/src/contrast.rs
  - apps/graphdraw/src/interaction.rs
  - apps/graphdraw/src/interaction2.rs
  - apps/graphdraw/src/ui/align_docker.rs
  - apps/graphdraw/src/ui/context.rs
  - apps/graphdraw/src/ui/svg_icons.rs
  - docs/behavior/align-distribute.md
---

## Decided
- Docker buttons draw colour SVGs from apps/graphdraw/assets/icons/color/ in their own colours (tint WHITE, faded white alpha 90 when disabled) via svg_icons::paint_color; texture cache key is "color/<stem>" so it never collides with the monochrome file of the same stem.
- split.mjs there cuts the drawing sheet (mm coordinates, two rows of six) into square 34-unit viewBox files; rerun it when the sheet changes.
- Gap buttons (SpacingH/V) and the Object menu keep the monochrome drawings: the sheet has no gap pictures.

## Tried and ruled out
- Putting colour files in FILES: every_embedded_svg_parses_and_draws_something asserts white-on-transparent, and the menu would change too.

## Watch out
- Cell ranges in split.mjs are by path bbox centre; a sheet with moved pictures needs new ranges.
