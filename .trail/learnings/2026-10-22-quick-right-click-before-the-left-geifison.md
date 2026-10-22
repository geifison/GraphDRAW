---
title: Quick right click before the left drag starts
author: Geifison
date: 2026-10-22
branch: main
---

## Decided
- The real cause of "a quick right-click copy takes the object behind": a right press before egui decides the left drag has started moves egui's press_origin to the right press point (often over the background). App::primary_press keeps the left press point and canvas_input starts the drag there; App::press_copy turns a right click before the drag starts into a copy; right_drag_input does not start a right-button drag while the left button is held.
- This is a different mechanism from the one ruled out in the earlier right-click learning (a drag restarting after the right button goes up), which stays ruled out.

## Tried and ruled out
- Fixing only the centre mark (an unselected object over it now wins the press): needed, but not enough for the quick click.

## Watch out
- Headless probes with small steps kept the right press over the dragged object and so passed; the reproducing test moves the pointer onto the background in the same frame as the right press.
