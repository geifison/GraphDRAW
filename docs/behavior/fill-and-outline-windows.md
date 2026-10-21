# Edit Fill and Outline Pen windows

Two floating windows edit the fill (F11, or a double click on the status
bar's fill) and the outline (F12, the Outline Pen tool, or a double click
on the status bar's outline) of the selected objects. With nothing
selected they edit the fill and outline new objects get (the Change
Defaults question follows OK, as for any change made with nothing
selected).

## Floating, with a live preview

- The windows do not block the program: they open at the top right of
  the screen, move by their title bar and keep their place while open.
  The drawing can be zoomed, scrolled and selected while they are open.
- While a window has changes, the drawing shows them on the objects it
  edits. The document itself changes only with OK, as one undo step (fill
  and overprint together; the outline of every object together). Cancel,
  Esc or the close button leave the drawing as it was.
- A new selection reloads a window that has no changes yet; a window with
  changes keeps editing the objects it was opened for.
- With the pointer over a window, its keys are not drawing shortcuts:
  Enter is OK, Esc is Cancel and Delete removes the chosen gradient node.
- "In the docker" opens the same settings in the Properties docker.

## Edit Fill

| Part | What it does |
|---|---|
| Modes | No fill, Uniform, Gradient, Vector pattern, Bitmap pattern, Two-color pattern, Texture; Mesh when the fill is a mesh already; PostScript shown disabled |
| Uniform | Fill transparency 0..100% (the fill alone; multiplies with the object transparency; saved, drawn and exported as fill opacity in SVG and PDF); colour picker inline (hue ring with saturation and brightness square, model list with sliders, hexadecimal, palettes with search, eyedropper); Overprint |
| Gradient types | Linear, Radial, Conical, Square, Shaped |
| Ramp | Double click on the band: new node with the gradient's colour there; double click on a middle node: removes it (the ends stay); drag a node to move it; Delete removes the chosen node; a palette colour dragged from the picker onto the band becomes a node |
| Node | Colour (pop-up picker), position (0..100%) |
| Presets | The 29 built-in gradients |
| Matrix | Angle with dial (Shift: 15 degree steps) for linear, conical and square; centre X and Y (-100..100%) for radial, conical and square; repeat mirrored or plain with the cycle count; edge pad (0..49%); bilinear; invert colours; Reverse (the first node goes to the end) |
| Patterns | Vector: "Use copied objects" makes the tile from the clipboard (Ctrl+C); Bitmap: "Choose image..."; Two colours: 8 tiles, front and back colours; tile width (and height for vector tiles) |
| Texture | Clouds, Marble, Wood, Noise; seed with Shuffle; noise scale (mm); front and back colours |
| Samples | New fill above the original one, over a chequerboard |

## Outline Pen

| Part | What it does |
|---|---|
| Has outline | Off removes the outline |
| Colour | Pop-up picker with eyedropper |
| Width | Typed with the unit inside the field (pt, mm, cm, in, px, m; 3 decimals), Presets (hairline, 0.1 to 36 pt), Hairline (0.0762 mm) |
| Line style | Solid, Dashed (4, 2), Long dash (8, 3), Dotted (1, 2, round caps), Dash-dot (5, 2, 1, 2), the user's styles; Edit... opens the line style editor (2..48 cells, one cell = one width; Use, Add, Replace, Close); user styles are kept in the settings |
| Corners | Miter, Round, Bevel |
| Caps | Flat, Round, Extended |
| Arrowheads | Start and end with thumbnails (the presets and the custom heads); Options... swaps start and end |
| Calligraphic pen | Nib view (drag the tip to turn, the side to flatten), stretch 1..100%, angle 0..360 degrees, Default |
| Variable width | Uniform, Taper end, Taper start, Taper both, Bulge; the shape follows a new width |
| Options | Behind fill, Scale with object, Overprint outline |

With several objects selected and an outline before, OK changes on each
object only the settings changed in the window.

## Next step

Shown dimmed with the tip "comes in the next step", because the drawing
does not keep these settings yet: outline transparency, gradient
node transparency and midpoints, steps, blending around the colour wheel,
smoothing, size, the elliptical type, editable two-colour cells, tile
scale, rotation, resolution, offsets, mirroring and "transform with
object", texture detail, contrast and granite, how dashes go through
corners, the miter limit, the outline position, and the arrowheads' size,
rotation, mirror and offset.
