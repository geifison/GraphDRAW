---
title: "Selected text shows no outline, only handles"
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 8, tokens: 14205 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/contrast.rs
  - docs/behavior/pick-tool.md
  - ".gemini/settings.json"
  - ".github/copilot-instructions.md"
  - GEMINI.md
  - dist/graphdraw-0.9.4-windows-x86_64-setup.exe
  - apps/graphdraw/src/text_editing.rs
  - crates/text/src/lib.rs
  - crates/text/src/on_path.rs
---

## Decided
User rule (2026-10-24): a selected text has no cyan line joining its handles. contrast::selection_outline returns nothing for any ShapeKind::Text (artistic: handles only; paragraph: its dashed frame, drawn with the frames). Curves and shapes keep their own outline.
## Tried and ruled out
The artistic text bounds box outline from the 2026-10-22 selection-marks learning: user asked to remove it.
## Watch out
docs/behavior/pick-tool.md "The handles" describes this; keep it in sync.
