---
title: "Dialogs scrolled past 420 px: egui default window height"
author: Geifison
date: 2026-10-23
branch: main
cost: { minutes: 36, tokens: 81809 }
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
- chrome::Window (apps/graphdraw/src/ui/chrome.rs) sets .default_height(screen height). egui gives every Window a default size of 340x420; with resizable(false) the body never gets more than that, so the chrome's "dialog_body" ScrollArea scrolled any dialog taller than ~390 px even on a big screen.
- Print dialog: tab area is dialogs::PRINT_BODY (760x340); the Layout tab puts the options left and the sheet sketch right (print_layout_grid / print_layout_sketch). print_layout_fields still stacks both for the print preview's left panel.
- Test: help_shots::print_dialog_tabs_fit_without_scrolling renders every tab in pt-BR at 1280x800, checks one size for all tabs and that the window holds title, tabs, body and buttons. PRINT_SHOTS=<dir> saves PNGs.

## Tried and ruled out
- Checking only that all tabs have equal height: passes while the window is clamped at 420 with the buttons cut off.
- Three frames per tab in the headless harness: an auto-sized window is not settled yet; use 8.

## Watch out
- Group frames (print_group) add ~12 px of margin to a width set with set_width; the column must subtract it or the dialog widens per tab.
- graphdraw-text test forced_justification_spreads_the_last_line_too failed on Windows before this change.
