---
title: "Print preview: where it lives and its status"
author: Geifison
date: 2026-10-23
branch: main
cost: { minutes: 1, tokens: 50045 }
touches:
  - AGENTS.md
  - ".gemini/settings.json"
  - ".github/copilot-instructions.md"
  - GEMINI.md
  - dist/graphdraw-0.9.4-windows-x86_64-setup.exe
  - apps/graphdraw/src/ui/print_preview.rs
  - docs/behavior/printing.md
  - docs/features.md
---

## Decided
- Print preview already exists: apps/graphdraw/src/ui/print_preview.rs (PrintPreview, show, render_sheet), opened by App::print_preview (app.rs). Sheets come from export::print_sheets, same as Print.
- Spec is docs/behavior/printing.md "## Print Preview"; status row in docs/features.md line ~274.

## Tried and ruled out
- Nothing implemented yet; only the survey. User wants to improve it and asked for a research pass first.

## Watch out
- docs/features.md contradicts itself: the header count lists print preview as "partial", the row says "works". No written list of what is missing.
- Only two tests cover it (plates_split_the_sheet_by_ink, the_settings_key_ignores_copies_and_preflight).
