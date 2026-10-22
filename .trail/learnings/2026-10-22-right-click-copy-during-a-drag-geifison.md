---
title: "Right-click copy during a drag: wrong object and two undo steps"
author: Geifison
date: 2026-10-22
branch: fix/right-click-clone
touches:
  - apps/graphdraw/src/interaction.rs
  - apps/graphdraw/src/pick_drag_tests.rs
  - docs/behavior/pick-tool.md
---

## Decided
- finish_transform wraps duplicate_in_place + transform_selection in App::as_one_step("Duplicate"), so Ctrl+Z removes the copy at once (before, undo only took back the move and the copy jumped onto the original).
- center_mark_at returns false when hit_test finds an unselected object over the mark (a small one on a selected background). The top-level group of a nested (Ctrl+click) selection does not count as covering it.

## Tried and ruled out
- egui re-starting a drag after the right button goes up (press_origin cleared on release): headless probes with many press/hold/release timings all copied the right object; not the cause.

## Watch out
- Headless tests use fixed time; timing-dependent pointer bugs may not reproduce there.
- A right press before egui decides the left drag started opens the drop menu and moves nothing (separate behaviour, left as is).
- graphdraw-text test forced_justification_spreads_the_last_line_too fails on Windows on the base commit, unrelated to this.
