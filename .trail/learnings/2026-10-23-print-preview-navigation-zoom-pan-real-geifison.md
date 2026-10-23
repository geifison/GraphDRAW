---
title: "Print preview navigation: zoom, pan, real size"
author: Geifison
date: 2026-10-23
branch: main
cost: { minutes: 29, tokens: 88784 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/help_shots.rs
  - apps/graphdraw/src/ui/chrome.rs
  - apps/graphdraw/src/ui/dialogs.rs
  - ".gemini/settings.json"
  - ".github/copilot-instructions.md"
  - GEMINI.md
  - dist/graphdraw-0.9.4-windows-x86_64-setup.exe
  - apps/graphdraw/lang/ar.toml
  - apps/graphdraw/lang/bn.toml
---

## Decided
- Preview view state lives in PrintPreview (zoom over fit, pan in points from view centre, marquee); pure helpers zoom_about, zoom_to_rect, clamp_pan, real_size_zoom are what the tests cover.
- Real size = screen_points_per_inch(settings)/25.4 points per mm over the fitted scale, the same calibration the drawing window's 100 % uses.
- Sheet bitmap: whole sheet up to 4096 px longest side; past it render_sheet draws only the on-screen part via the view origin, so zoom stays sharp.

## Tried and ruled out
- Keeping the ScrollArea: it can't zoom about the pointer or pan with the middle button; replaced by an own interact rect.

## Watch out
- trail check flags BLACK/WHITE asserts and guide prose as conflicts with the contrast-colour learnings: keyword false positives.
- On this Windows machine graphdraw-text forced_justification_spreads_the_last_line_too fails and help_shots print_dialog_tabs_fit_without_scrolling is flaky under the full parallel run (passes alone); neither touches the preview.
- No Python on this machine; use node for scripted edits of lang/*.toml (CRLF files).
