---
title: Selection lines are cyan; double-click finishes clip frame editing
author: Geifison
date: 2026-10-22
branch: graphdraw
cost: { minutes: 29, tokens: 81277 }
touches:
  - AGENTS.md
  - apps/graphdraw/lang/en.toml
  - apps/graphdraw/lang/pt-BR.toml
  - apps/graphdraw/src/app.rs
  - apps/graphdraw/src/canvas.rs
  - apps/graphdraw/src/clip_frames.rs
  - apps/graphdraw/src/clipboard.rs
  - apps/graphdraw/src/contrast.rs
  - apps/graphdraw/src/interaction.rs
  - apps/graphdraw/src/ops2.rs
---

## Decided
- Selection lines (solid outline, Shape tool dashed outline, chosen paragraph text frame, marquee, lasso) all go through contrast::draw_polyline, coloured by contrast::line_ink_on: cyan #00E5FF, neon blue #1F51FF where the backdrop is within RGB distance 120 of cyan, black/white (ink_on) when close to both. Handles stay black/white.
- No box is drawn between the handles; only the objects' own outlines.
- Double-click on the canvas with the Pick tool goes through clip_frame_edit_click (renamed from clip_frame_ctrl_click), the same rule as Ctrl+click: clip frame -> edit contents, outside the edited contents -> finish that level. It runs before the page-border check.

## Tried and ruled out
- WCAG contrast ratio to pick the line colour: cyan on white paper fails it, so it would never show cyan, which the user explicitly wants.

## Watch out
- The user may be running an old build when reporting visual issues; headless shots (help_shots.rs clip_frame_shots with CLIP_SHOTS=<dir>) show the current drawing.
- On Windows close graphdraw.exe before cargo build or the link step fails.
