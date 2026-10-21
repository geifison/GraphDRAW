# Outline Pen and Outline Colour

The Outline Pen tool (F12) opens the floating Outline Pen window (see
`fill-and-outline-windows.md`) and returns to the Pick tool; the same
settings stay in the Properties panel, Outline section. Outline
Colour (Shift+F12) opens the Colour panel. The model is
`graphdraw_core::Stroke`.

## Fields and defaults

| Field | Default | Notes |
| --- | --- | --- |
| Colour | black (C0 M0 Y0 K100 for new objects) | any colour model |
| Width | 0.2 mm (0.567 pt) for new objects; imported outlines keep theirs | a hairline (0.0762 mm, 0.216 pt) draws one device pixel at every zoom |
| Caps | butt | butt, round, square |
| Corners | mitre | mitre, round, bevel |
| Miter limit | 4 | 1 to 100 outline widths: a mitre longer than that is bevelled (`miter_limit`) |
| Dashes at corners | standard | standard, align, fixed (`dash_corners`) |
| Position | centred | centred, outside, inside; closed paths only (`position`) |
| Transparency | 0 % | the outline's own opacity 0..1 (`opacity`), times the object's |
| Dash | solid | presets dashed 4:2, dotted 1:1, dash dot 6:2:1:2, in multiples of the width |
| Arrowheads | none / none | start and end; presets and custom heads, filled in the outline colour |
| Arrowhead options | 100 %, 0 degrees, not mirrored, 0 mm | per end (`start_arrow_options`, `end_arrow_options`): size 5 to 5000 % of the preset size, turn about the tip, mirror across the curve's direction, the tip moved along the curve's direction by the offset |
| Nib stretch | 1.0 | 0.1 to 1: the calligraphic nib's thickness across its axis |
| Nib angle | 0 | degrees |
| Behind fill | off | the outline is drawn under the fill |
| Scale with object | off | the width follows the object's scale factor `sqrt(|det|)` |

Old files load with these defaults, and an outline that keeps them
writes none of the new fields, so `.gdr` files stay as they were.

## Position, dashes at corners, transparency

- Outside and inside: the outline is stroked twice as wide (dash
  lengths kept) and the path's inside is cut away (outside) or kept
  alone (inside) with the polygon operations of `shaping`, so the whole
  width lies on one side; the band is filled
  (`shaping::outline_band`). Open curves, hairlines and painterly
  outlines stay centred; a calligraphic or variable outline keeps its
  own band and ignores the position.
- Align and fixed: the path is cut into its dashes first
  (`shaping::corner_dashes`), restarting at every corner (a turn of
  more than 1 degree). Fixed keeps the exact lengths; align scales the
  pattern on each stretch so it holds whole repeats plus one dash,
  which puts a whole dash through each corner. The dashes on both sides
  of a corner form one piece, so the corner keeps its join. A closed
  path with no corner (a circle) is one loop. The pieces are stroked
  solid and filled as a band.
- Transparency: on screen the outline (stroke or band, and its
  arrowheads) is drawn into a layer of its own and laid over at its
  opacity, so the heads do not darken the line where they overlap.

Exports:

| Format | Transparency | Miter limit | Outside, inside, dashes at corners | Arrowhead options |
|---|---|---|---|---|
| SVG | `stroke-opacity`, `fill-opacity` on bands and heads | `stroke-miterlimit` when not 4 | filled band path | in the head outlines |
| PDF | ExtGState `CA`/`ca` (outline opacity times the object's) | `M` with every mitred outline | filled band | in the head outlines |
| EPS | left out (no transparency) | `setmiterlimit` | filled band | in the head outlines |
| EMF, WMF | left out | the format's default | filled band in the outline colour | in the head outlines |
| DXF | left out | not written | centred path | not written |
| `.cdr` | not written | not written | not written (centred, standard dashes) | not written (arrowheads are not written at all) |

Checked by `style::outline_option_tests`, `shaping::outline_option_tests`,
the renderer's `outline_option_tests`, `svg::tests::outline_options_reach_the_svg`,
`pdf::tests::outline_options_reach_the_pdf`,
`eps::tests::outline_options_reach_the_eps` and
`ui::outline_window::tests::merge_takes_the_new_outline_settings`.

## Calligraphic outline

- With stretch below 1 the outline is the area swept by an elliptical
  nib of `width` along the nib angle and `width * stretch` across it:
  each flattened segment contributes the parallelogram the nib's support
  sweeps (half-width `sqrt((a cos phi)^2 + (b sin phi)^2)` for the
  segment normal at angle `phi` to the nib axis), each vertex the nib
  itself, and the union is filled with the outline colour. Open paths
  end with nib-shaped caps; dashes are ignored.
- The same band is written by the SVG and PDF exports as a filled path,
  so the stroke looks the same outside the editor.
- A hairline is never calligraphic.

## Checks

- Given a 6 mm flat nib (stretch 0.1) at 0 degrees, a vertical line is
  6 mm wide and a horizontal line 0.6 mm tall.
- Given stretch 1, the band equals a 2 mm round-capped stroke.
- Given a rectangle with a 3 mm calligraphic outline, the SVG has the
  rectangle without a stroke and a filled band path in the outline
  colour.

## Outline paint

The Properties docker's outline section has Paint with every kind a fill
has, as the same row of pictures and the same editors: None, Uniform,
Gradient, Pattern, Texture and Mesh.

- None takes the outline away (as clearing the Outline box does).
- Uniform is the outline's colour (`Stroke::color`).
- The other kinds give the outline its own fill (`Stroke::fill`), edited
  as an object's fill is (for a gradient: type, built-in gradients,
  stops, repeat, bilinear, invert). It spreads over the outline's bounds
  (the path's bounds grown by half the width) and paints only the
  outline's area: the stroke, or the band of a calligraphic or variable
  outline. A mesh's nodes are local to the object, as a mesh fill's are;
  a shaped gradient follows the outline's own area (dark at its edges
  when the first colour is dark).
- Hairlines, arrowheads and painterly marks keep the outline colour.

Exports:

| Format | Outline with its own paint |
|---|---|
| SVG | `stroke` refers to the gradient or pattern (linear, radial; conical and square as linear, shaped as radial); a texture or mesh strokes with a representative colour (the mean of its colours). A calligraphic band is filled with the paint. |
| PDF | the outline's area (`shaping::stroke_area`: width, caps, joins, the miter limit, dashes) clipped with the non-zero rule and shaded (gradients) or painted with the renderer's pixels at 150 dpi (patterns, textures, meshes, shaped gradients). |
| EPS, EMF and the rest | the outline colour. |
| `.cdr` | the outline colour (outline fills are not written). |

Checked by `tests::outlines_take_a_gradient_fill` and
`tests::outlines_take_pattern_texture_mesh_and_shaped_paints` in the
renderer, `svg::tests::outline_paints_are_written_as_the_stroke_paint`,
`pdf::tests::gradient_options_and_outline_paints_reach_the_pdf`,
`shaping::tests::stroke_area_covers_the_outline_and_hairlines_cover_nothing`
and `ui::dockers::tests::outline_paint_offers_every_fill_kind`.

