# Gradient fill

## Controls and defaults

| Control | Default | Range |
|---|---|---|
| Type | Linear | Linear, Radial, Elliptical, Conical, Square, Shaped |
| Stops | 2: colour 0% = object colour or black, 100% = white | 2..64 |
| Stop opacity | 100% | 0..100% |
| Stop midpoint (of the run to the next stop) | 50% | 1..99% |
| Size (span against the bounds-fitted one) | 100% | 1..10000% |
| Axis offset (linear, along the axis) | 0 | half lengths of the automatic axis |
| Y radius (elliptical, square) | 100% of the x radius | 1..10000% |
| Steps (bands) | 0 = continuous | 2..256 |
| Smoothing (dither) | off | |
| Blend | Direct | Direct, around the colour wheel clockwise or counter-clockwise |
| Angle (linear, conical, square) | 0 (left to right, counter-clockwise positive) | -360..360 |
| Edge pad | 0% | 0..49% |
| Centre offset (radial, conical, square) | 0, 0 | -100..100% of half the bounding box |
| Steps (display) | 256 | 2..256 |
| Repeat | None | None, Sawtooth, Mirror |
| Cycles (with a repeat) | 2 when a repeat is first chosen | 1..50 |
| Bilinear | off | |
| Invert colours | off | |

The editor shows the whole run as drawn (repeats, mirror and inversion
in) in a strip under the type. The Interactive Fill property bar and the
Transparency tool's gradient list offer the Shaped type too; a shaped
gradient has no handles on the canvas.

Shortcut: F11 opens the floating Edit Fill window (see
`fill-and-outline-windows.md`); G is the Interactive Fill tool.
Dragging with Interactive Fill on an object (anywhere inside it, filled
or not) sets a linear fill from the press point to the release point; a
click gives the default fill; Ctrl constrains the angle to 15 degree
steps; dragging a palette colour onto a stop recolours it; dragging a
colour onto the fill line adds a stop.

Property bar (Interactive Fill): the fill type (None, Uniform, Gradient,
Pattern, Texture, Mesh), then the type's fields: the colour for a
uniform fill; for a gradient the kind (Linear, Radial, Conical, Square),
the start and end colours, the angle and the edge pad. With a selection
the fields edit it; with nothing selected they set the default fill for
the next object.

After the edge pad the bar has the size, the Y radius (elliptical and
square), steps (the lock turns them on at 16), the three blends,
smoothing and Reverse (positions and midpoints mirrored), all with the
Edit Fill window's icons; with a stop chosen, its colour, position,
opacity and midpoint. Every field is one undo step. The hint reads
"Drag the ends and stops; double-click the line to add a stop; click a
stop for its color; Delete removes it; Shift turns the axis; Ctrl = 15
degree steps".

Handles on the canvas (Interactive Fill, one gradient-filled object
selected; `gradient_tool.rs`, geometry in `graphdraw_core::gradient`).
The line between the two end handles (white squares) is the axis; the
stops sit on it as circles in their colours, the chosen one ringed.
Handles are taken within 7 screen points, the ends first, then the Y
radius, then the stops.

- Linear: the start and end of the axis, `c + u h (o -/+ s)` with
  `u = (cos a, sin a)`, `h = (w |cos a| + h |sin a|) / 2`, size `s` and
  axis offset `o`. Dragging one end moves that end alone (angle, size
  and offset follow; the other end stays); Shift turns the axis about
  the bounds' centre keeping its length; Ctrl turns in 15 degree steps.
  An end dragged off the centre line lands on it (the colours are the
  same).
- Radial, elliptical, conical, square: the start handle is the centre
  `c + offset * (w, h) / 2` (offset clamped to -1..1); the end handle the
  end of the x radius along the angle (square: along x). Dragging it sets
  the size (radial, square), the size and the angle (elliptical) or the
  angle (conical). Elliptical and square fills have a diamond at the end
  of the Y radius; dragging it sets the Y radius.
- Stops: `t = (e + p' (1 - 2e)) / n` along the axis (`p' = 1 - p` when
  inverted, `e` the edge pad, `n` the cycles, halved when bilinear).
  Dragging a stop slides it along the axis (0..1); on release the stops
  are sorted and the dragged one stays chosen.
- A double click on the axis adds a stop there with the colour and
  opacity the stops already give, and chooses it.
- A click on a stop chooses it (on an end: the end stop) and opens the
  stop balloon: the colour picker, opacity and midpoint, applied at
  once; OK, Esc or a click outside closes it. A click on the axis does
  nothing else; elsewhere it gives the default fill as before.
- Delete or Backspace removes the chosen stop (two always stay). A
  palette colour clicked goes to the chosen stop; a colour dragged from
  a palette shows the stops of the gradient under the pointer and,
  dropped on one, recolours it (the selection stays).
- Each change is one undo step; a drag is one step on release. After
  an undo the chosen stop no longer holds (Delete and the palette act
  on the object again).
- Dragging across an object (not on a handle) makes a linear fill from
  the press point to the release point (Ctrl: 15 degree steps), with the
  stops and options of its gradient, or its colour to white, or the
  default colour to white.

The older `fill_tool::drag_fountain_handle` (an end turns the axis, the
centre node sets the offset) stays for scripts and tests.

## Formulas

Bounding box `b` of the object in page space, centre `c`.

- Linear: axis from `c - h*(cos a, sin a)` to `c + h*(cos a, sin a)` with
  `h = (w*|cos a| + h*|sin a|) / 2`; `t = projection of p on the axis,
  normalised 0..1`.
- Radial: `t = |p - (c + offset)| / (max(w, h) / 2 * sqrt 2)`.
- Conical: `t = ((atan2(p - c') - a) mod 2pi) / 2pi`, mirrored so the
  colour at 0 and 1 is the same (`t = 1 - |2t - 1|`).
- Square: `t = max(|dx|, |dy|)` in the frame rotated by `a`, normalised
  by half the diagonal.
- Shaped: `t = d(p) / max d`, where `d` is the distance from `p` to the
  object's edge (0 outside) and `max d` the distance of the point
  deepest inside. The distances are exact Euclidean distances on a grid
  of at most 384 pixels across the object (`edge_distance` in the
  renderer, two passes of the 1-D lower envelope of parabolas), drawn
  smoothed; they do not change with zoom.
- The stop parameter `s` of `t` (`Fountain::stop_param`), in this order:
  1. Bilinear: `t = 1 - |2t - 1|` (first colour at both ends, last in
     the middle).
  2. Repeat with `n` cycles: `x = t * n`; Sawtooth `t = frac(x)` (1 at
     the very end); Mirror `t = frac(x)` on even cycles, `1 - frac(x)` on
     odd ones. No repeat: `n` is ignored.
  3. Edge pad `e`: `t = clamp((t - e) / (1 - 2e), 0, 1)` (per cycle).
  4. Invert colours: `s = 1 - t`.
- Elliptical: `t = sqrt((p'/rx)^2 + (q'/ry)^2)` in the frame turned by
  `a` about `c'`, `rx = w/2 sqrt 2`, `ry = h/2 sqrt 2 * aspect`. Square
  with a Y radius: `t = max(|dx|, |dy| / aspect) * 2` in bounds units.
- Span (`Fountain::span_param`), before the stop parameter: linear
  `x = (2t - 1 - o + s) / (2s)`; radial, elliptical, square `x = t / s`;
  conical and shaped unchanged. Inside 0..1 it is the parameter; beyond,
  with a size or axis offset set, Sawtooth repeats (`frac x`) and Mirror
  reflects; otherwise the end colours hold.
- Steps `N`: `s = (floor(s N) + 0.5) / N`, clamped to 0..1.
- Colour at `s` is a linear interpolation in RGB between the two stops
  around it, at `k = bias(local, m)` with `bias(k, m) = k^(ln 0.5 / ln m)`
  for the first stop's midpoint `m` (identity at 0.5); around the colour
  wheel the hue runs up (clockwise) or down, saturation and brightness
  straight. The opacity mixes the same way. Stops are sorted by
  position; equal positions give a hard edge.
- Smoothing adds up to 0.75 of a level of noise per pixel (a hash of the
  point) before quantising; such fills are drawn pixel by pixel.
- Gradient shaders and exports that know one run of stops take
  `Fountain::resolved_stops`: the same colours as stops over 0..1, with
  a sawtooth's jumps as hard edges (two stops at one position).

## Checks

- Given a 100x10 mm rectangle with stops black 0%, red 50%, white 100%
  and angle 0, when rendered at 1 px/mm, then pixel (50, 5) is red
  within 16/255 and pixel (2, 5) is black, pixel (97, 5) is white.
- Given a conical fill, when sampled at two angles 180 degrees apart,
  then the colours differ.
- Given a PDF export of a 3-stop fill, then the shading uses a
  stitching function with one bound at 0.3.
- Given a 40 x 20 mm rectangle with a 0 degree linear fill, then the
  handles are at (0, 10) and (40, 10); dragging the end node to (20, 30)
  makes the angle 90 degrees.
- Given a radial fill, dragging the centre node to (30, 15) on that
  rectangle sets the offset to (0.5, 0.5); beyond the edge it clamps to 1.
- Given a 100 x 10 mm black to white linear fill with Sawtooth, 2 cycles,
  rendered at 1 px/mm, then pixel 48 is light and pixel 51 dark; with
  Mirror, 2 cycles (or Bilinear), the middle is white and both ends black;
  with Invert, the left end is white
  (`tests::linear_fountains_repeat_mirror_and_invert`).
- Given a shaped black to white fill on that strip, then the edge is
  dark, the middle light, and points at the same distance from the edge
  have the same colour; on a circle, rings of equal colour
  (`tests::shaped_fountains_follow_the_outline`).
- `style::tests::resolved_stops_draw_the_same_colours_in_one_run` checks
  every combination of repeat, cycles, bilinear, invert and edge pad
  against `color_at`.

- `gradient::tests` (core) check the handle drags, adding, sorting and
  removing stops, midpoints, steps, the blends, opacity, the extras
  worked into `resolved_stops`, and their `.gdr` round trip;
  `tests::fountain_extras_are_drawn` (render) the pixels;
  `gradient_tool::tests::interactive_gradient_tool` the walk-through
  (new gradient, end drag, double click, palette colour, Delete, five
  undos) and `clicking_gradient_end_opens_stop_popup` the balloon.

## Files and exports

The newer options (`opacity` and `midpoint` per stop, `scale`,
`axis_offset`, `aspect`, `steps`, `smooth`, `blend`, kind `elliptical`)
are left out of `.gdr` files when at their defaults, so older files read
as before. `resolved_stops` works the size, axis offset, midpoints,
steps and blending into stops over the automatic span (sampled, hard
edges kept), so SVG, PDF, EPS, EMF and `.cdr` draw the same colours along
the automatic axis; SVG writes `stop-opacity`, the other formats drop the
opacity. An elliptical fill is written as a radial one; smoothing is for
the screen and pictures only.

The options are saved in `.gdr` files (`repeat`, `repeats`,
`bilinear`, `invert`, kind `shaped`); they are left out when off, so
files without them read as before and the format version stays.

| Format | Repeat, bilinear, invert | Shaped |
|---|---|---|
| SVG | worked into the stops | a radial gradient from the centre to the nearer sides, the inside colour at the centre |
| PDF | worked into the stitching function (hard edges leave out the empty segment) | the renderer's pixels at 150 dpi, clipped by the path |
| EPS, EMF | worked into the stops | a square gradient from the centre out |
| `.cdr` | the closest plain gradient (`Fountain::to_plain`): the options worked into at most 64 stops, edge pad 0 | written as a square gradient with the stops reversed (inside at the centre) |

The `.cdr` byte layout does not change; a gradient without the options
is written exactly as before
(`write::tests::gradient_options_are_written_as_the_closest_plain_gradient`).

## Importing a gradient

The fountain fill editor's Import Gradient button reads a `.ggr` file
(`graphdraw_io::ggr`) and replaces the stops with it. Plain linear
segments with their middle halfway give their two ends; segments with
another blending (curved, sine, sphere, step), a middle elsewhere or an
HSV colouring are sampled into 17 stops. End colours given as the
foreground or background colour read as black and white. Transparency
in the file is not kept (fountain stops are opaque).

## Built-in gradients

The fill editor's Gradients section starts with Current colours: the
last fill colour picked to the last outline colour picked (the palette's
left and right clicks, black and white until then; `App::current_colors`,
checked by `app::tests::palette_picks_set_the_current_colours`). Then
come 29 swatches (Black to white,
Sunset, Ocean, Rainbow, Gold, Silver, Copper, Chrome, Fire, Ice, Forest,
Sky, Dusk, Aurora, Pastel, Neon, Peach, Mint, Berry, Lavender, Desert,
Steel, Sepia, Charcoal, then five landscapes: Dawn over the sea, Desert
dusk, Forest horizon, Night horizon, Snowy mountains;
`gradient_presets.rs`). A click replaces the stops and keeps the type,
angle and centre. The landscapes run from the ground or sea (0) through
a horizon near the middle to the sky (1); their swatches are drawn
bottom to top, and a click on a linear fill also sets the angle to 90
degrees so the sky is on top. Checked by
`gradient_presets::tests::every_preset_runs_from_0_to_1_in_order` and
`gradient_presets::tests::landscapes_run_ground_to_sky_at_ninety_degrees`.
