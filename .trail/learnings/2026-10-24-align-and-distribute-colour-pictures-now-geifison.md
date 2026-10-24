---
title: Align and Distribute colour pictures now used in the menu too; one icon map rule
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 36, tokens: 63302 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/assets/icons/color/break-apart.svg
  - apps/graphdraw/assets/icons/color/combine.svg
  - apps/graphdraw/assets/icons/color/order-back.svg
  - apps/graphdraw/assets/icons/color/order-front.svg
  - apps/graphdraw/assets/icons/color/split-arrange.mjs
  - apps/graphdraw/src/app.rs
  - apps/graphdraw/src/contrast.rs
  - apps/graphdraw/src/main.rs
---

## Decided
- User rule (2026-10-24): a changed icon changes everywhere. GLYPHS map Align/Distribute to `color/<stem>`, so the Object menu and the docker show the same colour picture; the docker's own colour table (color_picture, paint_color, COLOR_KEYS) is gone, it draws via menu_glyph + paint_svg.
- svg_icons::color_tint gives colour pictures WHITE when asked in Tokens::icon(), white alpha 90 otherwise.
- Test a_changed_icon_changes_everywhere fails if any table, FILES or MICRO still names a monochrome stem that has a colour picture. Rule written in AGENTS.md "Icons".
## Tried and ruled out
- Keeping the Object menu monochrome (the earlier Align learning's choice): superseded by the user's rule.
## Watch out
- Colour align-center-h has the horizontal axis and belongs to Glyph::AlignCenterH; the old monochrome mapping was crossed (CenterH -> align-center-v).
