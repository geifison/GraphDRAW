---
title: "Clip frames take more objects, keep contents when copied"
author: Geifison
date: 2026-10-22
branch: graphdraw
cost: { minutes: 16, tokens: 136003 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/app.rs
  - apps/graphdraw/src/canvas.rs
  - apps/graphdraw/src/clip_frames.rs
  - apps/graphdraw/src/clipboard.rs
  - apps/graphdraw/src/main.rs
  - apps/graphdraw/src/ops2.rs
  - apps/graphdraw/src/ui/eyedropper_ui.rs
  - apps/graphdraw/src/ui/mod.rs
  - crates/core/src/command.rs
---

## Decided
- Command::PlaceInside on a frame that is already a ClipFrame appends to its contents (absorbing the inverse of parent * clip transform) instead of wrapping it in a second ClipFrame. The frame may be nested (found with find_shape_mut / parent_transform); contents still come from a layer.
- Placing a frame's own ancestor into it is a no-op.
- Objects created while editing clip frame contents join that level: App.clip_frame_edit_from holds, per level, the first shape id given out after editing began; clip_edit_ids() = extracted ids + top-level page shapes with id >= that mark. Used by clip_edit_allows, the canvas veil and finish_clip_frame_edit.
- Copy, duplicate, selection_svg and the native clipboard payload use selected_shapes_whole (no text_view), so a text clip frame keeps its contents.

## Tried and ruled out
- Changing the clip_frame_edit tuple type: too many consumers (canvas, bar, menus); a parallel stack was simpler.

## Watch out
- selected_shapes() maps clip_frames::text_view, which drops contents of text clip frames: never use it for copies.
- The repo is not cargo-fmt clean overall; format only your own lines.
