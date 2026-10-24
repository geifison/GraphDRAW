---
title: Double click on a text edits it with any tool
author: Geifison
date: 2026-10-24
branch: main
cost: { minutes: 11, tokens: 85776 }
touches:
  - AGENTS.md
  - apps/graphdraw/src/anchors.rs
  - apps/graphdraw/src/contrast.rs
  - apps/graphdraw/src/interaction.rs
  - apps/graphdraw/src/text_editing.rs
  - docs/behavior/pick-tool.md
  - docs/behavior/shape-tool.md
  - docs/behavior/text.md
  - docs/guide/text.md
  - docs/help/en/text/adding-text.md
---

## Decided
- User rule (2026-10-24): a quick double click on a text opens it for editing with any tool, not only Pick.
- One check in canvas_input (apps/graphdraw/src/interaction.rs) before the per-tool match: double_click_text(p). It selects the text and calls edit_selected_text (switches to the Text tool, caret at the end), same as the Pick tool.

## Tried and ruled out
- Adding the case inside each tool's input function: many tools, easy to miss one; one shared check is simpler.

## Watch out
- Tools whose own double click must win: Pick/FreeformPick and Text (handle it themselves), Eraser (deletes object), curve with 2+ nodes in progress (finishes it), Crop inside crop_area (applies), AnchorEditing on a custom anchor (removes it). Keep this list in double_click_text in sync when a tool gains a double click.
- The first click of the double click still runs the tool's single click (Zoom zooms in, Bezier places a node that gets dropped, Eraser erases a dot and can turn the text into curves).
- Docs: docs/behavior/text.md "Editing in place", shape-tool.md, guide/text.md, help en and pt-BR adding-text.md. Other help languages still say Pick only.
- Test: text_editing::tests::a_double_click_on_a_text_edits_it_with_any_tool.
