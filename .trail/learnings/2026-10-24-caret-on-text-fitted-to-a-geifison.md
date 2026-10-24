---
title: Caret on text fitted to a path
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 11, tokens: 106301 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/text_editing.rs
  - crates/text/src/lib.rs
  - crates/text/src/on_path.rs
  - docs/behavior/text.md
  - ".gemini/settings.json"
  - ".github/copilot-instructions.md"
  - GEMINI.md
  - dist/graphdraw-0.9.4-windows-x86_64-setup.exe
---

## Decided
on_path::place (crates/text/src/on_path.rs) also returns, per line, each LineBox edge mapped onto the path (point on the baseline + unit "up"): TextLayout::path_carets. An edge goes with the glyph it falls in, same distance from that glyph's start as on the straight line, so corners/justify/relax are followed. TextLayout::path_caret and hit_char use it; app text_editing::caret_geometry_sized branches to path_caret_geometry (one selection quad per char). Glyph placement itself is untouched.
## Tried and ruled out
Mapping the straight caret through to_page only: on-path glyphs are re-placed inside the layout, so to_page alone keeps the caret straight.
## Watch out
Track flattens curves: tangents are chord directions, a caret on a circle is ~0.01-0.02 rad off radial; tests need an angle tolerance. forced_justification_spreads_the_last_line_too fails on Windows before this change (fonts).
