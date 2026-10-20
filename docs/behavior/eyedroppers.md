# Colour Eyedropper and Attributes Eyedropper

Both tools have two modes shown on the property bar: Select (sample)
and Apply. A sample switches to Apply; the Select button or Esc goes
back. The two tools are the two modes of the eyedropper: Colour (only
the colour value) and Attributes (the object's properties). The first
buttons of either property bar switch between them, and switching
empties the bucket.

## Sample and pour

Without leaving the tool:

1. The pipette: a click samples. The pointer is a pipette.
2. Right after, the pointer becomes a bucket and the bucket shows what
   it holds (the property bar, the magnifier).
3. The bucket: a click applies to the object under the pointer. Every
   application is one undo step.
4. Shift+click samples again without applying; the bucket stays active
   (handy for filling the palette).
5. Alt+click switches pipette and bucket without sampling or applying.
6. Esc empties the bucket and goes back to the pipette, staying in the
   tool. A second Esc (nothing in the bucket) drops back to the Pick
   tool. The Empty button on the property bar does the same as the first
   Esc.

Targets: the usual hit test, else the topmost unfilled closed object
whose interior contains the point, so an empty rectangle can be filled
by clicking inside it. Without Ctrl a group receives as a whole (it
passes the fill or outline on to everything in it); Ctrl+click reaches
the object inside the group. Sampling always reads the innermost object.

## Colour Eyedropper

- Property bar: the two modes, Select Color, Apply Color, the sample
  size, Outside the Window (not available yet, greyed out with the
  reason: it needs a system screen capture), the bucket (a swatch with
  its description, or the strip of a cloned ramp), Empty (Esc), and the
  gesture hint.
- Sample size: Native (default), 1x1, 2x2, 3x3, 5x5.
  - Native: the value of the object's own model, without anti-aliasing:
    a solid fill gives its colour; a fountain fill gives the colour of
    the exact point of its ramp; outside the fill (on the outline) the
    outline colour; a mesh fill its nearest node; a bitmap the pixel
    under the pointer; empty areas the screen pixel.
  - N x N: a bitmap gives its pixels averaged over the box (box anchored
    at the pointer pixel and growing right and down when even, clipped
    to the image), as RGB; elsewhere the screen pixels of the last
    drawing averaged over the same box, laid over the paper or the
    desktop where the drawing is transparent. Without a drawing (no
    window yet) the native value is used.
- The right button in the pipette phase reads the outline colour first.
- Applying, left button:
  - a closed object: a solid fill with the colour (`SetFill`);
  - an open line (when the document does not fill open curves): the
    outline colour;
  - text: the whole object; the letters' own fills are dropped (one
    batch, one undo step);
  - on a fountain stop (within 6 screen pixels): only that stop takes
    the colour; while the colour bucket is over an object with a
    fountain fill, its stops are drawn as squares in their colours (the
    one under the pointer larger).
- Applying, right button: the outline colour, the fill stays; it opens
  no context menu. An object with no outline gets the outline new
  objects get (the document's graphic defaults), in that colour
  (`SetStroke`).
- Alt+click on a fountain fill in the pipette phase clones the whole
  ramp (stops, positions, angle, kind, repeats). A click pours it on the
  target's fill; fountains are laid out on each object's own bounds, so
  the axis runs from edge to edge of the target. The right button with a
  ramp changes nothing and says the ramp goes to the fill.
- Fountain point formulas, on the object's page bounds `b` (centre `c`):
  - linear, angle `a`: half span `h = (w |cos a| + h |sin a|) / 2`,
    `t = (p - (c - h d)) . d / 2h` with `d = (cos a, sin a)`;
  - radial: `t = |p - c'| / r`, `c'` the centre with its offset,
    `r = max(w, h) / 2 * sqrt 2`;
  - conical and square: as the renderer, on `u, v` relative to `b`.
  Then the colour is `Fountain::color_at(t)` (repeats, mirror, edge pad
  and inversion included). Stops are placed at the first run of the
  colours: on the linear axis; from the centre to the right for radial
  and square; around the centre for conical.

## Magnifier

While an eyedropper is active a floating magnifier follows the pointer
(24 px away, on the other side near the edges of the window):

- an 11 x 11 grid of screen pixels magnified 10 times, with lines
  between the pixels and a white and black frame around the sample box;
- the colour under the pointer: a swatch, HEX, CMYK (a CMYK colour shows
  its own values) and RGB 0 to 255, and the name of the colour style of
  the same colour, when there is one;
- the bucket;
- the last 5 tones sampled as circles, newest first and without
  repeats; a click on one loads the bucket.

When the pointer rests for 0.4 s the magnifier stops and waits so the
pointer can reach it; it follows again once the pointer is 40 px outside
it. The right button on the grid adds the colour to the document palette
(one undo step, nothing when it is there already).

When the bucket is poured the target gets a 150 ms highlight around its
bounds, growing by 6 px and fading out.

## Attributes Eyedropper

- Property bar: the two modes, Select Object Attributes, Apply Object
  Attributes, the Properties menu (Outline, Fill, Text; all on by
  default), the Transformations menu (Size, Rotation, Position, Skew;
  off by default), Effects (off by default), Empty (Esc) and the hints.
- Select mode, click: reads from the innermost object: fill, extra
  fills, outline, extra outlines, the first text span's font (family,
  size, bold, italic, tracking), the paragraph spacing (leading and its
  unit, character and word spacing), bounds width and height, rotation
  of the transform (angle of its x axis), its skew, bounds centre,
  opacity, drop shadow and live effects.
- Apply mode, click (Shift+click samples again instead; Alt switches;
  Ctrl reaches inside groups): one batch "Copy Attributes" with the
  ticked groups. The object the sample came from is never changed.
  - Fill and Outline: `SetFill`, `SetStroke`, and the extra fills or
    outlines (`SetExtraPaint`). Bitmaps keep their own. Copying a fill
    to text drops the letters' own fills.
  - Text: when both objects are text, every span of the target takes
    the sampled font and tracking and the target takes the paragraph
    spacing; the text itself is kept.
  - Effects: opacity, drop shadow and the live effect list.
  - Skew, Rotation, Size, Position, in that order about the target's
    bounds centre: give the transform the sampled shear (keeping its
    rotation and scale), rotate by the angle difference, scale the
    bounds to the sampled width and height, then move the centre to the
    sampled position. One `TransformShapes` carries them.
  - Skew: the linear part is `R(angle) * [[a, b], [0, d]]`; the shear
    is `b / d`.

## Checks

- Given a red rectangle and an empty one, when the colour eyedropper
  clicks the red one then inside the empty one, then the empty one has
  a solid red fill in one undo step; a right click afterwards gives it a
  red outline.
- Pipette on red, click on blue: blue becomes red; right click on green:
  green's outline becomes red and its fill stays green; Shift+click on
  green: the drawing does not change and the bucket holds green; Alt+click
  twice: pipette, then bucket, no undo step; Esc: the bucket is empty.
- Alt+click on a 100 mm linear fountain, click on a 400 x 200 mm
  rectangle: the axis runs from edge to edge of the rectangle.
- Given a 10 x 20 mm blue rectangle at 50 % opacity and a 10 x 10 mm
  empty one, with default groups the second gets the blue fill and keeps
  its size and opacity; with Size and Effects ticked it becomes 10 x 20
  mm about its own centre at 50 %.
