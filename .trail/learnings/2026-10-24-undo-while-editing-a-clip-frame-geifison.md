---
title: Undo while editing a clip frame stays inside the frame
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 30, tokens: 213918 }
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
- Entering clip frame editing runs ExtractContents (an undo step). Undo while editing must work as outside but never leave the frame: App::undo_stops_at_clip_frame blocks undo when Engine::undo_revision() <= the revision recorded at entry (App::clip_frame_edit_rev, one per nested level, pushed in edit_clip_frame, popped in finish_clip_frame_edit). Status key status.undo_clip_frame_start; can_undo reflects it.
## Tried and ruled out
- Ending the edit level when undo reaches the extraction: user wants to stay inside the frame (stated 2026-10-24).
## Watch out
- Old bug cause: canvas hides the frame being edited (raster.hidden), so undoing ExtractContents made the frame and its contents vanish.
- Engine rev of a step is per entry (HistoryEntry.rev), robust to max_history trimming, unlike stack length.
