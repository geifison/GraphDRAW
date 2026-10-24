---
title: "Colour pictures for To Front, To Back, Combine, Break Apart"
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 28, tokens: 111882 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/lang/ar.toml
  - apps/graphdraw/lang/bn.toml
  - apps/graphdraw/lang/cs.toml
  - apps/graphdraw/lang/de.toml
  - apps/graphdraw/lang/en.toml
  - apps/graphdraw/lang/es.toml
  - apps/graphdraw/lang/fr.toml
  - apps/graphdraw/lang/hi.toml
---

## Decided
- The four Pics map to `color/<stem>` in PICS (svg_icons.rs), so every caller of paint/paint_svg (property bar, Object menu, context menu) draws the colour file from assets/icons/color/.
- paint_svg passes `color_tint(name, color)`: colour pictures get WHITE when asked in Tokens::icon(), white alpha 90 otherwise (disabled); monochrome drawings keep `color`.
- Sheet split by assets/icons/color/split-arrange.mjs (2x2 grid at x 110 / y 140 mm, black captions dropped, shared square side).
- The monochrome order-front/order-back/combine/break-apart stems left FILES (the "every embedded file is mapped" test requires it); make-icons.mjs still writes them.
## Tried and ruled out
- Keeping the "shows blue" check in the colour test: Combine is all dark ink; the check is now "opaque non-white pixel".
## Watch out
- help_shots::print_dialog_tabs_fit_without_scrolling can fail under the full parallel run and pass alone.
- The dark ink #211e1f reads poorly on a dark theme.
