---
title: "Print prepress guide: roadmap for print preview work"
author: Geifison
date: 2026-10-23
branch: main
cost: { minutes: 2, tokens: 68994 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/help_shots.rs
  - apps/graphdraw/src/ui/dialogs.rs
  - ".gemini/settings.json"
  - ".github/copilot-instructions.md"
  - GEMINI.md
  - dist/graphdraw-0.9.4-windows-x86_64-setup.exe
  - docs/print-prepress-guide.md
  - docs/behavior/printing.md
  - docs/features.md
---

## Decided
- docs/print-prepress-guide.md is the guidance doc for improving Print, Print Preview and preflight; it maps prepress practice to current GraphDraw status per section, with a priority list in section 12.
- Written in English like the rest of docs/; brand and other-product feature names stripped (AGENTS.md rule 3), only standards kept (ISO 12647, FOGRA39, GRACoL/SWOP, CIP4 JDF).
- It is guidance, not spec: a feature taken from it still needs docs/behavior/printing.md rules and a docs/features.md row.

## Tried and ruled out
- Copying the user's pasted Portuguese source verbatim: it named other products and had loose technical claims (screen angles, fixed 65 % dot gain), so it was rewritten.

## Watch out
- Preview navigation rows marked "to check" (wheel zoom, middle-button pan) were not verified in print_preview.rs.
- New print options must go into export::print_sheets, never the preview alone, so Print and Preview stay identical.
