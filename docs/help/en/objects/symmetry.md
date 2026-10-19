# Drawing with symmetry

Symmetry draws live mirrored copies of an object across one or more mirror
lines. The copies follow every change you make to the original, so you draw
one half (or one slice) and see the whole. The commands are in
[[menu.object > menu.object.symmetry]].

## To create a symmetry {#create}

1. Select an object.
2. Choose [[menu.object > menu.object.symmetry > menu.object.symmetry_create]]
   (<kbd>Alt+S</kbd>).

A vertical mirror line is placed at the right edge of the object, so the copy
appears next to it, and GraphDraw goes straight into editing the symmetry.

## Editing a symmetry {#edit}

[[menu.object.symmetry_edit]] (<kbd>Alt+E</kbd>) edits the symmetry of the
selected object. The original is outlined in red, and a bar at the top left of
the drawing window sets:

- the number of mirror lines, 1 to 12, spread evenly around the center. With
  more lines you get a kaleidoscope: with *n* lines there are 2*n* - 1 copies;
- the center (X and Y);
- the angle of the first line (0 to 360 degrees).

Edit the original as usual, with any tool; the copies follow. While an object
with symmetry is selected, its mirror lines show as dashed lines.

Click Finish on the bar, or choose [[menu.object.symmetry_finish]]
(<kbd>Alt+Shift+E</kbd>), to stop editing.

## Removing or keeping the copies {#remove}

- [[menu.object.symmetry_remove]] (<kbd>Alt+Shift+S</kbd>) removes the
  symmetry; the copies go away.
- [[menu.object.symmetry_break]] (<kbd>Alt+X</kbd>) turns the copies into
  objects of their own, which no longer follow the original.
