---
title: Right-click on a locked object offers Unlock
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 20, tokens: 84608 }
touches:
  - AGENTS.md
  - ATTRIBUTION.md
  - apps/graphdraw/src/app.rs
  - apps/graphdraw/src/contrast.rs
  - apps/graphdraw/src/interaction.rs
  - apps/graphdraw/src/interaction2.rs
  - apps/graphdraw/src/ui/align_docker.rs
  - apps/graphdraw/src/ui/context.rs
  - apps/graphdraw/src/ui/svg_icons.rs
  - docs/behavior/align-distribute.md
---

## Decided
hit_test skips locked shapes, so a right click on one left the selection empty and opened page_menu with no unlock. open_context_menu (interaction2.rs) now falls back to App::locked_at(p) (interaction.rs) after hit and guide checks, stores the id in App::context_locked, and page_menu (ui/context.rs) puts Unlock Object (unlocks and selects it) and Unlock All Objects first. context_locked is cleared when the menu closes.

## Tried and ruled out
Selecting the locked shape so object_menu shows Unlock: selection code assumes selectable shapes (handles, transforms), kept a separate field instead.

## Watch out
menus::replay skips disabled rows (Paste with an empty clipboard), so row index tests must not expect a disabled row; compare row counts instead. An unlocked object under a locked one still wins the right click.
