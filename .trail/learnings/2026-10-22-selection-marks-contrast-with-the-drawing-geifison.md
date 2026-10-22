---
title: Selection marks contrast with the drawing under them
author: Geifison
date: 2026-10-22
branch: graphdraw
cost: { minutes: 7, tokens: 107540 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/canvas.rs
  - apps/graphdraw/src/main.rs
  - apps/graphdraw/src/ui/eyedropper_ui.rs
  - apps/graphdraw/src/ui/mod.rs
  - ".gemini/settings.json"
  - ".github/copilot-instructions.md"
  - GEMINI.md
  - apps/graphdraw/src/contrast.rs
  - dist/graphdraw-0.9.4-windows-x86_64-setup.exe
---

## Decided
- apps/graphdraw/src/contrast.rs: handles, selection hairlines, paragraph text frames and the text cursor pick black or white from the kept page pixels (Raster::kept) composited over paper/desktop, by perceived brightness.
- Raster keep_pixels is on while something is selected, a text/table cell is edited, or an eyedropper is active (contrast::keep_pixels), set before draw_canvas and in canvas_overlay.
- Pick tools: solid 1px outline; Shape tool: dashed. Artistic text outlines its bounds box; paragraph text uses its frame; curves use their path.
- Caret blinks (0.53 s half period) via egui temp data keyed by caret position; restarts on move.

## Tried and ruled out
- Inverting the exact background colour: mid grey gives grey, unreadable. Black/white by luminance instead.

## Watch out
- Outlines skip when the selection flattens to more than 40k points.
- Kept pixels lag one render when selection first appears (re-render forced by key check).
